use super::*;
use crate::{
    ports_test_helpers::{FakePorts, LOGS_DIR, spec},
    start::start_apps,
};

#[tokio::test]
async fn deleting_a_running_app_terminates_it_and_drops_the_record() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    start_apps(&mut table, &[spec("api")], LOGS_DIR, &ports)
        .await
        .expect("start should succeed");
    let outcome = delete_app(&mut table, &AppSelector::Id(0), &ports)
        .await
        .expect("delete should succeed");
    assert_eq!(outcome.name, "api");
    assert_eq!(outcome.force_kill_pid, Some(100));
    assert_eq!(ports.terminated(), vec![100]);
    assert!(table.records().is_empty());
}

#[tokio::test]
async fn deleting_a_stopped_app_needs_no_signal() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    table.upsert(spec("api"), 1000);
    let outcome = delete_app(&mut table, &AppSelector::Name("api".to_string()), &ports)
        .await
        .expect("delete should succeed");
    assert_eq!(outcome.force_kill_pid, None);
    assert!(table.records().is_empty());
}

#[tokio::test]
async fn deleting_an_unknown_selector_reports_not_found() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    let err = delete_app(&mut table, &AppSelector::Id(9), &ports)
        .await
        .unwrap_err();
    assert!(matches!(err, UsecaseError::NotFound(_)), "got: {err}");
}

#[tokio::test]
async fn a_persistence_failure_propagates() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    table.upsert(spec("api"), 1000);
    ports.fail_save();
    let err = delete_app(&mut table, &AppSelector::Id(0), &ports)
        .await
        .unwrap_err();
    assert!(matches!(err, UsecaseError::Dump(_)), "got: {err}");
}

#[tokio::test]
async fn a_signal_failure_propagates() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    start_apps(&mut table, &[spec("api")], LOGS_DIR, &ports)
        .await
        .expect("start should succeed");
    ports.fail_signal_for(100);
    let err = delete_app(&mut table, &AppSelector::Id(0), &ports)
        .await
        .unwrap_err();
    assert!(matches!(err, UsecaseError::Signal(_)), "got: {err}");
}

#[tokio::test]
async fn deleting_persists_the_shrunken_table() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    table.upsert(spec("api"), 1000);
    delete_app(&mut table, &AppSelector::Id(0), &ports)
        .await
        .expect("delete should succeed");
    assert!(ports.stored().is_empty());
}
