use usecases::{DumpError, SpecError, StartOutcome};

use super::{test_helpers::*, *};
use crate::{apps_file::AppsFileError, http::HEALTH_OK, process_views::running_view};

const APPS_FILE: &str = "/srv/pm3/apps.yaml";

fn listed_nothing() -> DaemonReply {
    DaemonReply::Listed(Vec::new())
}

fn acknowledged(name: &str) -> DaemonReply {
    DaemonReply::Stopped {
        name: name.to_string(),
    }
}

fn start_body() -> String {
    format!("{{\"apps_file\":\"{APPS_FILE}\"}}")
}

#[tokio::test]
async fn health_reports_that_the_daemon_is_up() {
    let (status, json) = exchange_json(get_from("/health")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["status"], HEALTH_OK);
}

#[tokio::test]
async fn starting_apps_forwards_the_apps_file() {
    let outcome = Ok(DaemonReply::Started(Vec::new()));
    let exchange = exchange(outcome, post_to("/apps", &start_body())).await;
    assert_eq!(
        exchange.request,
        Some(DaemonRequest::Start {
            apps_file: APPS_FILE.to_string(),
        })
    );
}

#[tokio::test]
async fn starting_apps_returns_the_start_summary() {
    let outcome = Ok(DaemonReply::Started(vec![StartOutcome {
        pm_id: 0,
        name: "web".to_string(),
        pid: Some(4242),
        already_running: false,
    }]));
    let exchange = exchange(outcome, post_to("/apps", &start_body())).await;
    assert_eq!(exchange.body, "started web (id 0, pid 4242)");
}

#[tokio::test]
async fn listing_apps_asks_the_daemon_for_the_whole_table() {
    let exchange = exchange(Ok(listed_nothing()), get_from("/apps")).await;
    assert_eq!(exchange.request, Some(DaemonRequest::List));
}

#[tokio::test]
async fn listing_apps_returns_the_rendered_table() {
    let outcome = Ok(DaemonReply::Listed(vec![running_view(0, "web")]));
    let exchange = exchange(outcome, get_from("/apps")).await;
    assert!(exchange.body.contains("web"), "got: {}", exchange.body);
}

#[tokio::test]
async fn a_numeric_selector_is_read_as_an_id() {
    let outcome = Ok(DaemonReply::Described(running_view(3, "web")));
    let exchange = exchange(outcome, get_from("/apps/3")).await;
    assert_eq!(
        exchange.request,
        Some(DaemonRequest::Describe(AppSelector::Id(3)))
    );
}

#[tokio::test]
async fn a_textual_selector_is_read_as_a_name() {
    let outcome = Ok(DaemonReply::Described(running_view(3, "web")));
    let exchange = exchange(outcome, get_from("/apps/web")).await;
    assert_eq!(
        exchange.request,
        Some(DaemonRequest::Describe(AppSelector::Name(
            "web".to_string()
        )))
    );
}

#[tokio::test]
async fn describing_an_app_returns_its_details() {
    let outcome = Ok(DaemonReply::Described(running_view(3, "web")));
    let exchange = exchange(outcome, get_from("/apps/web")).await;
    assert!(
        exchange.body.contains("/usr/bin/node"),
        "got: {}",
        exchange.body
    );
}

#[tokio::test]
async fn stopping_an_app_forwards_the_selector() {
    let exchange = exchange(Ok(acknowledged("web")), post_to("/apps/web/stop", "")).await;
    assert_eq!(
        exchange.request,
        Some(DaemonRequest::Stop(AppSelector::Name("web".to_string())))
    );
}

#[tokio::test]
async fn stopping_an_app_confirms_the_app() {
    let exchange = exchange(Ok(acknowledged("web")), post_to("/apps/web/stop", "")).await;
    assert_eq!(exchange.body, "stopped web");
}

#[tokio::test]
async fn restarting_an_app_forwards_the_selector() {
    let outcome = Ok(DaemonReply::Restarted {
        name: "web".to_string(),
    });
    let exchange = exchange(outcome, post_to("/apps/web/restart", "")).await;
    assert_eq!(
        exchange.request,
        Some(DaemonRequest::Restart(AppSelector::Name("web".to_string())))
    );
}

#[tokio::test]
async fn deleting_an_app_forwards_the_selector() {
    let outcome = Ok(DaemonReply::Deleted {
        name: "web".to_string(),
    });
    let exchange = exchange(outcome, delete_at("/apps/web")).await;
    assert_eq!(
        exchange.request,
        Some(DaemonRequest::Delete(AppSelector::Name("web".to_string())))
    );
}

#[tokio::test]
async fn an_unknown_app_answers_not_found() {
    let outcome = Err(UsecaseError::NotFound("web".to_string()).into());
    let exchange = exchange(outcome, get_from("/apps/web")).await;
    assert_eq!(exchange.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_rejected_spec_answers_bad_request() {
    let outcome = Err(UsecaseError::Spec(SpecError::EmptyName).into());
    let exchange = exchange(outcome, post_to("/apps", &start_body())).await;
    assert_eq!(exchange.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_rejected_spec_explains_itself_in_the_body() {
    let outcome = Err(UsecaseError::Spec(SpecError::EmptyName).into());
    let exchange = exchange(outcome, post_to("/apps", &start_body())).await;
    assert!(
        exchange.body.contains("blank app name"),
        "got: {}",
        exchange.body
    );
}

#[tokio::test]
async fn a_failed_state_write_answers_server_error() {
    let outcome = Err(UsecaseError::Dump(DumpError::Write {
        path: "/srv/pm3/dump.yaml".to_string(),
        reason: "disk full".to_string(),
    })
    .into());
    let exchange = exchange(outcome, post_to("/apps", &start_body())).await;
    assert_eq!(exchange.status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn an_unreadable_apps_file_answers_bad_request() {
    let outcome = Err(AppsFileError::NoApps.into());
    let exchange = exchange(outcome, post_to("/apps", &start_body())).await;
    assert_eq!(exchange.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_daemon_that_stopped_listening_answers_unavailable() {
    let exchange = exchange_without_a_daemon(get_from("/apps")).await;
    assert_eq!(exchange.status, StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn a_daemon_that_abandoned_the_request_answers_unavailable() {
    let exchange = exchange_with_an_abandoned_request(get_from("/apps")).await;
    assert_eq!(exchange.status, StatusCode::SERVICE_UNAVAILABLE);
}
