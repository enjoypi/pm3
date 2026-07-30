use super::*;
use crate::{
    presenter::EMPTY_NOTICE,
    process_views::{RUNNING_PID, idle_view, running_view},
};

fn started(name: &str, already_running: bool) -> StartOutcome {
    StartOutcome {
        pm_id: 3,
        name: name.to_string(),
        pid: Some(RUNNING_PID),
        already_running,
    }
}

#[test]
fn starting_nothing_says_so() {
    assert_eq!(render_started(&[]), NOTHING_STARTED);
}

#[test]
fn a_freshly_started_app_reports_its_id_and_pid() {
    let rendered = render_started(&[started("web", false)]);
    assert_eq!(rendered, format!("started web (id 3, pid {RUNNING_PID})"));
}

#[test]
fn an_already_running_app_is_left_alone() {
    let rendered = render_started(&[started("web", true)]);
    assert_eq!(
        rendered,
        format!("web is already running (id 3, pid {RUNNING_PID})")
    );
}

#[test]
fn an_app_that_failed_to_report_a_pid_shows_a_dash() {
    let outcome = StartOutcome {
        pid: None,
        ..started("web", false)
    };
    assert_eq!(render_started(&[outcome]), "started web (id 3, pid -)");
}

#[test]
fn every_started_app_gets_its_own_line() {
    let outcomes = vec![started("web", false), started("db", false)];
    assert_eq!(render_started(&outcomes).lines().count(), 2);
}

#[test]
fn a_start_reply_renders_the_start_summary() {
    let reply = DaemonReply::Started(vec![started("web", false)]);
    assert!(render_reply(&reply).starts_with("started web"));
}

#[test]
fn a_list_reply_renders_the_table() {
    let reply = DaemonReply::Listed(vec![running_view(0, "web")]);
    assert_eq!(
        render_reply(&reply),
        render_table(&[running_view(0, "web")])
    );
}

#[test]
fn an_empty_list_reply_renders_the_empty_notice() {
    assert_eq!(render_reply(&DaemonReply::Listed(Vec::new())), EMPTY_NOTICE);
}

#[test]
fn a_describe_reply_renders_the_details() {
    let reply = DaemonReply::Described(idle_view(0, "web"));
    assert_eq!(render_reply(&reply), render_describe(&idle_view(0, "web")));
}

#[test]
fn a_stop_reply_confirms_the_app() {
    let reply = DaemonReply::Stopped {
        name: "web".to_string(),
    };
    assert_eq!(render_reply(&reply), "stopped web");
}

#[test]
fn a_restart_reply_confirms_the_app() {
    let reply = DaemonReply::Restarted {
        name: "web".to_string(),
    };
    assert_eq!(render_reply(&reply), "restarted web");
}

#[test]
fn a_delete_reply_confirms_the_app() {
    let reply = DaemonReply::Deleted {
        name: "web".to_string(),
    };
    assert_eq!(render_reply(&reply), "deleted web");
}
