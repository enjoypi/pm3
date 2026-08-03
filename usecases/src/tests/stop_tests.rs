use entities::ProcessStatus;

use super::*;
use crate::{
    ports::ExitOutcome,
    ports_test_helpers::{FakePorts, LOGS_DIR, spec, spec_with_deps},
    restart::restart_app,
    start::start_apps,
    supervise::{ExitAction, handle_child_exit},
};

fn stopped_names(stopped: &[StopOutcome]) -> Vec<String> {
    stopped.iter().map(|outcome| outcome.name.clone()).collect()
}

async fn started_table(ports: &FakePorts) -> ProcessTable {
    let mut table = ProcessTable::new();
    start_apps(&mut table, &[spec("api")], LOGS_DIR, ports).await;
    table
}

#[tokio::test]
async fn stopping_a_running_app_sends_sigterm_and_marks_it_stopping() {
    let ports = FakePorts::new(1000);
    let mut table = started_table(&ports).await;
    let outcome = stop_app(&mut table, &AppSelector::Id(0), &ports)
        .await
        .expect("stop should succeed");
    assert_eq!(outcome.force_kill_pid, Some(100));
    assert_eq!(ports.terminated(), vec![100]);
    let record = table.find(&AppSelector::Id(0)).expect("record present");
    assert_eq!(record.runtime.status, ProcessStatus::Stopping);
    assert_eq!(record.runtime.pid, Some(100));
}

#[tokio::test]
async fn a_broken_dependency_graph_still_stops_every_service() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    start_apps(
        &mut table,
        &[spec("api"), spec_with_deps("web", &["api"])],
        LOGS_DIR,
        &ports,
    )
    .await;
    table.remove(&AppSelector::Name("api".to_string()));

    let stopped = stop_all_apps(&mut table, &ports)
        .await
        .expect("an unorderable table must not silently stop nothing");

    assert_eq!(stopped_names(&stopped), vec!["web".to_string()]);
}

#[tokio::test]
async fn a_broken_dependency_graph_still_signals_the_survivors() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    start_apps(
        &mut table,
        &[spec("api"), spec_with_deps("web", &["api"])],
        LOGS_DIR,
        &ports,
    )
    .await;
    table.remove(&AppSelector::Name("api".to_string()));

    stop_all_apps(&mut table, &ports)
        .await
        .expect("an unorderable table must not silently stop nothing");

    assert_eq!(ports.terminated(), vec![101]);
}

#[tokio::test]
async fn a_handover_names_the_service_that_is_still_draining() {
    let ports = FakePorts::new(1000);
    let mut table = started_table(&ports).await;
    stop_app(&mut table, &AppSelector::Id(0), &ports)
        .await
        .expect("stop should succeed");

    let draining = persist_for_handover(&table, &ports)
        .await
        .expect("a handover should succeed");

    assert_eq!(draining, vec!["api".to_string()]);
}

#[tokio::test]
async fn a_handover_keeps_a_draining_service_stopping_so_the_next_daemon_can_settle_it() {
    let ports = FakePorts::new(1000);
    let mut table = started_table(&ports).await;
    stop_app(&mut table, &AppSelector::Id(0), &ports)
        .await
        .expect("stop should succeed");

    persist_for_handover(&table, &ports)
        .await
        .expect("a handover should succeed");

    let record = table.find(&AppSelector::Id(0)).expect("record present");
    assert_eq!(record.runtime.status, ProcessStatus::Stopping);
}

#[tokio::test]
async fn a_handover_keeps_the_pid_of_a_draining_service() {
    let ports = FakePorts::new(1000);
    let mut table = started_table(&ports).await;
    stop_app(&mut table, &AppSelector::Id(0), &ports)
        .await
        .expect("stop should succeed");

    persist_for_handover(&table, &ports)
        .await
        .expect("a handover should succeed");

    let record = table.find(&AppSelector::Id(0)).expect("record present");
    assert_eq!(record.runtime.pid, Some(100));
}

#[tokio::test]
async fn a_handover_names_no_service_while_everything_runs() {
    let ports = FakePorts::new(1000);
    let table = started_table(&ports).await;
    let draining = persist_for_handover(&table, &ports)
        .await
        .expect("a handover should succeed");
    assert!(draining.is_empty());
}

#[tokio::test]
async fn a_handover_reports_a_dump_it_cannot_write() {
    let ports = FakePorts::new(1000);
    let table = started_table(&ports).await;
    ports.fail_save();
    let err = persist_for_handover(&table, &ports).await.unwrap_err();
    assert!(matches!(err, UsecaseError::Dump(_)), "got: {err}");
}

#[tokio::test]
async fn stopping_an_already_stopped_app_sends_no_signal() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    table.upsert(spec("api"), 1000);
    let outcome = stop_app(&mut table, &AppSelector::Name("api".to_string()), &ports)
        .await
        .expect("stop should succeed");
    assert_eq!(outcome.force_kill_pid, None);
    assert!(ports.terminated().is_empty());
}

#[tokio::test]
async fn a_running_record_without_a_pid_settles_as_stopped() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    table.upsert(spec("api"), 1000);
    let selector = AppSelector::Name("api".to_string());
    let live = table.find_mut(&selector).expect("record present");
    live.runtime.mark_launched(7, 1000);
    live.runtime.pid = None;
    let outcome = stop_app(&mut table, &selector, &ports)
        .await
        .expect("stop should succeed");
    assert_eq!(outcome.force_kill_pid, None);
    let settled = table.find(&selector).expect("record present");
    assert_eq!(settled.runtime.status, ProcessStatus::Stopped);
}

#[tokio::test]
async fn stopping_an_app_that_is_already_stopping_keeps_its_pid() {
    let ports = FakePorts::new(1000);
    let mut table = started_table(&ports).await;
    stop_app(&mut table, &AppSelector::Id(0), &ports)
        .await
        .expect("first stop should succeed");
    let outcome = stop_app(&mut table, &AppSelector::Id(0), &ports)
        .await
        .expect("second stop should succeed");
    assert_eq!(outcome.force_kill_pid, Some(100));
    let record = table.find(&AppSelector::Id(0)).expect("record present");
    assert_eq!(record.runtime.pid, Some(100));
    assert_eq!(record.runtime.status, ProcessStatus::Stopping);
}

#[tokio::test]
async fn stopping_an_unknown_selector_reports_not_found() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    let err = stop_app(&mut table, &AppSelector::Id(9), &ports)
        .await
        .unwrap_err();
    assert!(matches!(err, UsecaseError::NotFound(_)), "got: {err}");
}

#[tokio::test]
async fn a_signal_failure_propagates() {
    let ports = FakePorts::new(1000);
    ports.fail_signal_for(100);
    let mut table = started_table(&ports).await;
    let err = stop_app(&mut table, &AppSelector::Id(0), &ports)
        .await
        .unwrap_err();
    assert!(matches!(err, UsecaseError::Signal(_)), "got: {err}");
}

#[tokio::test]
async fn a_persistence_failure_propagates() {
    let ports = FakePorts::new(1000);
    let mut table = started_table(&ports).await;
    ports.fail_save();
    let err = stop_app(&mut table, &AppSelector::Id(0), &ports)
        .await
        .unwrap_err();
    assert!(matches!(err, UsecaseError::Dump(_)), "got: {err}");
}

#[tokio::test]
async fn stopping_everything_walks_dependents_before_dependencies() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    let specs = [spec_with_deps("web", &["api"]), spec("api")];
    start_apps(&mut table, &specs, LOGS_DIR, &ports).await;
    let stopped = stop_all_apps(&mut table, &ports)
        .await
        .expect("stop all should succeed");
    assert_eq!(
        stopped_names(&stopped),
        vec!["web".to_string(), "api".to_string()]
    );
}

#[tokio::test]
async fn stopping_everything_persists_the_table_once() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    let specs = [spec_with_deps("web", &["api"]), spec("api")];
    start_apps(&mut table, &specs, LOGS_DIR, &ports).await;
    let saves_after_start = ports.save_count();
    stop_all_apps(&mut table, &ports)
        .await
        .expect("stop all should succeed");
    assert_eq!(ports.save_count(), saves_after_start + 1);
}

#[tokio::test]
async fn stopping_everything_reports_nothing_when_all_apps_are_settled() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    table.upsert(spec("api"), 1000);
    let stopped = stop_all_apps(&mut table, &ports)
        .await
        .expect("stop all should succeed");
    assert!(stopped.is_empty());
}

#[tokio::test]
async fn stopping_everything_keeps_going_when_one_signal_is_refused() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    let specs = [spec_with_deps("web", &["api"]), spec("api")];
    start_apps(&mut table, &specs, LOGS_DIR, &ports).await;
    ports.fail_signal_for(100);
    let stopped = stop_all_apps(&mut table, &ports)
        .await
        .expect("stop all should succeed");
    assert_eq!(stopped_names(&stopped), vec!["web".to_string()]);
    assert_eq!(ports.terminated(), vec![101]);
}

#[tokio::test]
async fn a_persistence_failure_while_stopping_everything_propagates() {
    let ports = FakePorts::new(1000);
    let mut table = started_table(&ports).await;
    ports.fail_save();
    let err = stop_all_apps(&mut table, &ports).await.unwrap_err();
    assert!(matches!(err, UsecaseError::Dump(_)), "got: {err}");
}

#[tokio::test]
async fn stopping_persists_the_table() {
    let ports = FakePorts::new(1000);
    let mut table = started_table(&ports).await;
    stop_app(&mut table, &AppSelector::Id(0), &ports)
        .await
        .expect("stop should succeed");
    assert_eq!(ports.save_count(), 2);
}

#[tokio::test]
async fn stopping_a_service_cancels_a_queued_restart() {
    let ports = FakePorts::new(1000);
    let mut table = started_table(&ports).await;
    restart_app(&mut table, &AppSelector::Id(0), LOGS_DIR, &ports)
        .await
        .expect("restart should succeed");

    stop_app(&mut table, &AppSelector::Id(0), &ports)
        .await
        .expect("stop should succeed");

    let record = table.find(&AppSelector::Id(0)).expect("record present");
    assert!(!record.runtime.pending_restart);
}

#[tokio::test]
async fn a_stopped_service_stays_down_when_the_draining_process_finally_exits() {
    let ports = FakePorts::new(1000);
    let mut table = started_table(&ports).await;
    restart_app(&mut table, &AppSelector::Id(0), LOGS_DIR, &ports)
        .await
        .expect("restart should succeed");
    stop_app(&mut table, &AppSelector::Id(0), &ports)
        .await
        .expect("stop should succeed");

    let exit = ExitOutcome::Code(143);
    let action = handle_child_exit(&mut table, "api", exit, &ports)
        .await
        .expect("exit handled");

    assert_eq!(
        action,
        ExitAction::Settled {
            status: ProcessStatus::Stopped,
        }
    );
}

#[tokio::test]
async fn stopping_everything_cancels_a_queued_restart() {
    let ports = FakePorts::new(1000);
    let mut table = started_table(&ports).await;
    restart_app(&mut table, &AppSelector::Id(0), LOGS_DIR, &ports)
        .await
        .expect("restart should succeed");

    stop_all_apps(&mut table, &ports)
        .await
        .expect("stop all should succeed");

    let record = table.find(&AppSelector::Id(0)).expect("record present");
    assert!(!record.runtime.pending_restart);
}
