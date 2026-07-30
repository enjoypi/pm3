use entities::ProcessStatus;

use super::*;
use crate::{
    ports_test_helpers::{FakePorts, LOGS_DIR, spec},
    start::start_apps,
};

async fn started_table(ports: &FakePorts) -> ProcessTable {
    let mut table = ProcessTable::new();
    start_apps(&mut table, &[spec("api")], LOGS_DIR, ports)
        .await
        .expect("start should succeed");
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
async fn stopping_persists_the_table() {
    let ports = FakePorts::new(1000);
    let mut table = started_table(&ports).await;
    stop_app(&mut table, &AppSelector::Id(0), &ports)
        .await
        .expect("stop should succeed");
    assert_eq!(ports.save_count(), 2);
}
