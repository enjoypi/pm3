use entities::AppSpec;

use super::*;
use crate::{
    ports::SpecResolveError,
    ports_test_helpers::{FakePorts, LOGS_DIR, spec, spec_with_deps},
    start::start_apps,
    supervision::SupervisionRequest,
};

const KILL_TIMEOUT_MS: u64 = 1600;
const READY_TIMEOUT_MS: u64 = 30000;
const READY_POLL_MS: u64 = 200;

struct StaticResolver(Option<&'static str>);

impl StaticResolver {
    fn always() -> Self {
        Self(None)
    }

    fn failing(name: &'static str) -> Self {
        Self(Some(name))
    }
}

impl SpecResolver for StaticResolver {
    async fn prepare(&self, name: &str) -> Result<AppSpec, SpecResolveError> {
        if self.0 == Some(name) {
            return Err(SpecResolveError::Missing {
                name: name.to_string(),
                reason: "the declaration vanished".to_string(),
            });
        }
        Ok(spec(name))
    }
}

fn supervisor() -> Supervisor {
    Supervisor::new(
        LOGS_DIR.to_string(),
        KILL_TIMEOUT_MS,
        READY_TIMEOUT_MS,
        READY_POLL_MS,
    )
}

async fn two_app_supervisor(ports: &FakePorts) -> Supervisor {
    let mut supervisor = supervisor();
    start_apps(
        &mut supervisor.table,
        &[spec("web"), spec("api")],
        LOGS_DIR,
        ports,
    )
    .await;
    supervisor
}

fn reply_names(outcome: &SupervisionOutcome) -> Vec<String> {
    let reply = outcome.as_ref().expect("a batch reply");
    let names = match reply {
        SupervisionReply::StoppedAll { names }
        | SupervisionReply::RestartedAll { names }
        | SupervisionReply::DeletedAll { names }
        | SupervisionReply::ResetAll { names } => names,
        SupervisionReply::Started { .. }
        | SupervisionReply::Listed(_)
        | SupervisionReply::Described(_)
        | SupervisionReply::Stopped { .. }
        | SupervisionReply::Restarted { .. }
        | SupervisionReply::Deleted { .. }
        | SupervisionReply::Reset { .. }
        | SupervisionReply::Signalled { .. } => {
            panic!("expected a batch reply, got: {reply:?}")
        }
    };
    let mut sorted = names.clone();
    sorted.sort_unstable();
    sorted
}

#[tokio::test]
async fn stop_all_via_the_selector_stops_every_app() {
    let ports = FakePorts::new(1000);
    let mut supervisor = two_app_supervisor(&ports).await;
    let (outcome, _effects) = supervisor
        .handle(
            SupervisionRequest::Stop(AppSelector::All),
            &StaticResolver::always(),
            &ports,
        )
        .await;
    assert_eq!(reply_names(&outcome), ["api", "web"]);
}

#[tokio::test]
async fn restart_all_restarts_every_app() {
    let ports = FakePorts::new(1000);
    let mut supervisor = two_app_supervisor(&ports).await;
    let (outcome, _effects) = supervisor
        .handle(
            SupervisionRequest::Restart(AppSelector::All),
            &StaticResolver::always(),
            &ports,
        )
        .await;
    assert_eq!(reply_names(&outcome), ["api", "web"]);
}

#[tokio::test]
async fn restart_all_skips_an_app_whose_declaration_no_longer_resolves() {
    let ports = FakePorts::new(1000);
    let mut supervisor = two_app_supervisor(&ports).await;
    let (outcome, _effects) = supervisor
        .handle(
            SupervisionRequest::Restart(AppSelector::All),
            &StaticResolver::failing("api"),
            &ports,
        )
        .await;
    assert_eq!(reply_names(&outcome), ["web"]);
}

#[tokio::test]
async fn restart_all_continues_past_an_app_that_refuses_to_stop() {
    let ports = FakePorts::new(1000);
    let mut supervisor = two_app_supervisor(&ports).await;
    ports.fail_signal_for(100);
    let (outcome, _effects) = supervisor
        .handle(
            SupervisionRequest::Restart(AppSelector::All),
            &StaticResolver::always(),
            &ports,
        )
        .await;
    assert_eq!(reply_names(&outcome), ["api"]);
}

#[tokio::test]
async fn restart_all_on_an_empty_table_restarts_nothing() {
    let ports = FakePorts::new(1000);
    let mut supervisor = supervisor();
    let (outcome, _effects) = supervisor
        .handle(
            SupervisionRequest::Restart(AppSelector::All),
            &StaticResolver::always(),
            &ports,
        )
        .await;
    assert_eq!(reply_names(&outcome), Vec::<String>::new());
}

#[tokio::test]
async fn delete_all_removes_every_app() {
    let ports = FakePorts::new(1000);
    let mut supervisor = two_app_supervisor(&ports).await;
    let (outcome, _effects) = supervisor
        .handle(
            SupervisionRequest::Delete(AppSelector::All),
            &StaticResolver::always(),
            &ports,
        )
        .await;
    assert_eq!(reply_names(&outcome), ["api", "web"]);
    assert!(supervisor.table.names_in_table_order().is_empty());
}

#[tokio::test]
async fn delete_all_continues_past_an_app_that_refuses_to_stop() {
    let ports = FakePorts::new(1000);
    let mut supervisor = two_app_supervisor(&ports).await;
    ports.fail_signal_for(100);
    let (outcome, _effects) = supervisor
        .handle(
            SupervisionRequest::Delete(AppSelector::All),
            &StaticResolver::always(),
            &ports,
        )
        .await;
    assert_eq!(reply_names(&outcome), ["api"]);
    assert_eq!(supervisor.table.names_in_table_order(), ["web"]);
}

#[tokio::test]
async fn delete_all_falls_back_to_table_order_when_the_graph_cannot_sort() {
    let ports = FakePorts::new(1000);
    let mut supervisor = two_app_supervisor(&ports).await;
    supervisor
        .table
        .find_by_name_mut("web")
        .expect("web")
        .spec
        .depends_on = vec!["api".to_string()];
    supervisor
        .table
        .find_by_name_mut("api")
        .expect("api")
        .spec
        .depends_on = vec!["web".to_string()];
    let (outcome, _effects) = supervisor
        .handle(
            SupervisionRequest::Delete(AppSelector::All),
            &StaticResolver::always(),
            &ports,
        )
        .await;
    assert_eq!(reply_names(&outcome), ["api", "web"]);
}

#[tokio::test]
async fn delete_all_on_an_empty_table_deletes_nothing() {
    let ports = FakePorts::new(1000);
    let mut supervisor = supervisor();
    let (outcome, _effects) = supervisor
        .handle(
            SupervisionRequest::Delete(AppSelector::All),
            &StaticResolver::always(),
            &ports,
        )
        .await;
    assert_eq!(reply_names(&outcome), Vec::<String>::new());
}

#[tokio::test]
async fn reset_all_clears_every_app() {
    let ports = FakePorts::new(1000);
    let mut supervisor = two_app_supervisor(&ports).await;
    let (outcome, _effects) = supervisor
        .handle(
            SupervisionRequest::Reset(AppSelector::All),
            &StaticResolver::always(),
            &ports,
        )
        .await;
    assert_eq!(reply_names(&outcome), ["api", "web"]);
}

#[tokio::test]
async fn reset_all_continues_past_a_save_failure() {
    let ports = FakePorts::new(1000);
    let mut supervisor = two_app_supervisor(&ports).await;
    ports.fail_save();
    let (outcome, _effects) = supervisor
        .handle(
            SupervisionRequest::Reset(AppSelector::All),
            &StaticResolver::always(),
            &ports,
        )
        .await;
    assert_eq!(reply_names(&outcome), Vec::<String>::new());
}

#[tokio::test]
async fn dependency_order_names_records_in_a_deletable_order() {
    let ports = FakePorts::new(1000);
    let mut supervisor = supervisor();
    start_apps(
        &mut supervisor.table,
        &[spec_with_deps("web", &["api"]), spec("api")],
        LOGS_DIR,
        &ports,
    )
    .await;
    let (outcome, _effects) = supervisor
        .handle(
            SupervisionRequest::Delete(AppSelector::All),
            &StaticResolver::always(),
            &ports,
        )
        .await;
    assert_eq!(reply_names(&outcome), ["api", "web"]);
}

#[tokio::test]
async fn a_single_restart_reloads_the_declaration() {
    let ports = FakePorts::new(1000);
    let mut supervisor = two_app_supervisor(&ports).await;
    let (outcome, _effects) = supervisor
        .handle(
            SupervisionRequest::Restart(AppSelector::Name("web".to_string())),
            &StaticResolver::always(),
            &ports,
        )
        .await;
    let reply = outcome.expect("a restart reply");
    assert!(
        matches!(reply, SupervisionReply::Restarted { ref name } if name == "web"),
        "got: {reply:?}"
    );
}

#[tokio::test]
async fn a_single_restart_fails_when_the_declaration_vanishes() {
    let ports = FakePorts::new(1000);
    let mut supervisor = two_app_supervisor(&ports).await;
    let (outcome, _effects) = supervisor
        .handle(
            SupervisionRequest::Restart(AppSelector::Name("api".to_string())),
            &StaticResolver::failing("api"),
            &ports,
        )
        .await;
    assert!(outcome.is_err(), "got: {outcome:?}");
}

#[tokio::test]
async fn a_single_restart_fails_when_the_app_refuses_to_stop() {
    let ports = FakePorts::new(1000);
    let mut supervisor = two_app_supervisor(&ports).await;
    ports.fail_signal_for(100);
    let (outcome, _effects) = supervisor
        .handle(
            SupervisionRequest::Restart(AppSelector::Name("web".to_string())),
            &StaticResolver::always(),
            &ports,
        )
        .await;
    assert!(outcome.is_err(), "got: {outcome:?}");
}

#[tokio::test]
async fn a_single_stop_retires_the_app() {
    let ports = FakePorts::new(1000);
    let mut supervisor = two_app_supervisor(&ports).await;
    let (outcome, _effects) = supervisor
        .handle(
            SupervisionRequest::Stop(AppSelector::Name("web".to_string())),
            &StaticResolver::always(),
            &ports,
        )
        .await;
    let reply = outcome.expect("a stop reply");
    assert!(
        matches!(reply, SupervisionReply::Stopped { ref name } if name == "web"),
        "got: {reply:?}"
    );
}

#[tokio::test]
async fn a_single_stop_fails_when_the_app_is_unknown() {
    let ports = FakePorts::new(1000);
    let mut supervisor = two_app_supervisor(&ports).await;
    let (outcome, _effects) = supervisor
        .handle(
            SupervisionRequest::Stop(AppSelector::Name("ghost".to_string())),
            &StaticResolver::always(),
            &ports,
        )
        .await;
    assert!(outcome.is_err(), "got: {outcome:?}");
}

#[tokio::test]
async fn a_single_reset_clears_the_counters() {
    let ports = FakePorts::new(1000);
    let mut supervisor = two_app_supervisor(&ports).await;
    let (outcome, _effects) = supervisor
        .handle(
            SupervisionRequest::Reset(AppSelector::Name("web".to_string())),
            &StaticResolver::always(),
            &ports,
        )
        .await;
    let reply = outcome.expect("a reset reply");
    assert!(
        matches!(reply, SupervisionReply::Reset { ref name } if name == "web"),
        "got: {reply:?}"
    );
}

#[tokio::test]
async fn a_single_reset_fails_when_the_table_cannot_be_saved() {
    let ports = FakePorts::new(1000);
    let mut supervisor = two_app_supervisor(&ports).await;
    ports.fail_save();
    let (outcome, _effects) = supervisor
        .handle(
            SupervisionRequest::Reset(AppSelector::Name("web".to_string())),
            &StaticResolver::always(),
            &ports,
        )
        .await;
    assert!(outcome.is_err(), "got: {outcome:?}");
}
