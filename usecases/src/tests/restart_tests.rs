use entities::ProcessStatus;

use super::*;
use crate::{
    ports_test_helpers::{FakePorts, LOGS_DIR, spec},
    start::start_apps,
};

#[tokio::test]
async fn restarting_a_stopped_app_starts_it_immediately() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    table.upsert(spec("api"), 1000);
    let outcome = restart_app(&mut table, &AppSelector::Id(0), LOGS_DIR, &ports)
        .await
        .expect("restart should succeed");
    let RestartOutcome::Started(started) = outcome else {
        panic!("expected an immediate start");
    };
    assert_eq!(started.pid, Some(100));
    assert!(ports.terminated().is_empty());
}

#[tokio::test]
async fn restarting_a_running_app_terminates_it_and_records_the_intent() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    start_apps(&mut table, &[spec("api")], LOGS_DIR, &ports).await;
    let outcome = restart_app(&mut table, &AppSelector::Id(0), LOGS_DIR, &ports)
        .await
        .expect("restart should succeed");
    let RestartOutcome::AwaitingExit {
        name,
        force_kill_pid,
    } = outcome
    else {
        panic!("expected the restart to await the exit");
    };
    assert_eq!(name, "api");
    assert_eq!(force_kill_pid, Some(100));
    assert_eq!(ports.terminated(), vec![100]);
    let record = table.find(&AppSelector::Id(0)).expect("record present");
    assert_eq!(record.runtime.status, ProcessStatus::Stopping);
    assert!(record.runtime.pending_restart);
}

#[tokio::test]
async fn restarting_a_running_record_without_a_pid_needs_no_force_kill() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    table.upsert(spec("api"), 1000);
    let selector = AppSelector::Id(0);
    let record = table.find_mut(&selector).expect("record present");
    record.runtime.mark_launched(7, 1000);
    record.runtime.pid = None;
    let outcome = restart_app(&mut table, &selector, LOGS_DIR, &ports)
        .await
        .expect("restart should succeed");
    let RestartOutcome::AwaitingExit {
        name: _,
        force_kill_pid,
    } = outcome
    else {
        panic!("expected the restart to await the exit");
    };
    assert_eq!(force_kill_pid, None);
}

#[tokio::test]
async fn restarting_an_app_that_is_already_stopping_spawns_no_second_instance() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    start_apps(&mut table, &[spec("api")], LOGS_DIR, &ports).await;
    let selector = AppSelector::Id(0);
    restart_app(&mut table, &selector, LOGS_DIR, &ports)
        .await
        .expect("first restart should succeed");
    let outcome = restart_app(&mut table, &selector, LOGS_DIR, &ports)
        .await
        .expect("second restart should succeed");
    assert!(
        matches!(outcome, RestartOutcome::AwaitingExit { .. }),
        "got: {outcome:?}"
    );
    assert_eq!(ports.spawned_names(), vec!["api".to_string()]);
    let record = table.find(&selector).expect("record present");
    assert_eq!(record.runtime.pid, Some(100));
}

#[tokio::test]
async fn restarting_an_unknown_selector_reports_not_found() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    let err = restart_app(&mut table, &AppSelector::Id(9), LOGS_DIR, &ports)
        .await
        .unwrap_err();
    assert!(matches!(err, UsecaseError::NotFound(_)), "got: {err}");
}

#[tokio::test]
async fn a_persistence_failure_while_restarting_a_stopped_app_still_reports_the_spawn() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    table.upsert(spec("api"), 1000);
    ports.fail_save();
    let outcome = restart_app(&mut table, &AppSelector::Id(0), LOGS_DIR, &ports)
        .await
        .expect("a spawned process must be reported even when the dump cannot be written");
    let RestartOutcome::Started(started) = outcome else {
        panic!("a settled app is restarted by spawning it: {outcome:?}");
    };
    assert!(started.pid.is_some());
}

#[tokio::test]
async fn a_persistence_failure_while_restarting_a_running_app_still_reports_the_force_kill_pid() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    start_apps(&mut table, &[spec("api")], LOGS_DIR, &ports).await;
    ports.fail_save();
    let outcome = restart_app(&mut table, &AppSelector::Id(0), LOGS_DIR, &ports)
        .await
        .expect("a terminated process must be reported even when the dump cannot be written");
    let RestartOutcome::AwaitingExit { force_kill_pid, .. } = outcome else {
        panic!("a running app is restarted by stopping it first: {outcome:?}");
    };
    assert!(force_kill_pid.is_some());
}

#[tokio::test]
async fn a_signal_failure_while_restarting_propagates() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    start_apps(&mut table, &[spec("api")], LOGS_DIR, &ports).await;
    ports.fail_signal_for(100);
    let err = restart_app(&mut table, &AppSelector::Id(0), LOGS_DIR, &ports)
        .await
        .unwrap_err();
    assert!(matches!(err, UsecaseError::Signal(_)), "got: {err}");
}

#[tokio::test]
async fn a_failure_to_start_a_stopped_app_propagates() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    table.upsert(spec("api"), 1000);
    ports.fail_spawn_for("api");
    let err = restart_app(&mut table, &AppSelector::Id(0), LOGS_DIR, &ports)
        .await
        .unwrap_err();
    assert!(matches!(err, UsecaseError::Launch(_)), "got: {err}");
}

#[tokio::test]
async fn restarting_persists_the_table() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    table.upsert(spec("api"), 1000);
    restart_app(&mut table, &AppSelector::Id(0), LOGS_DIR, &ports)
        .await
        .expect("restart should succeed");
    assert_eq!(ports.save_count(), 1);
}
