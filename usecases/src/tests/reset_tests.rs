use entities::ProcessStatus;

use super::*;
use crate::ports_test_helpers::{FakePorts, started_table};

#[tokio::test]
async fn resetting_a_known_app_clears_its_restart_counters() {
    let ports = FakePorts::new(1000);
    let mut table = started_table(&ports).await;
    let seeded = table.find_mut(&AppSelector::Id(0)).expect("record present");
    seeded.runtime.count_restart(4);

    let name = reset_app(&mut table, &AppSelector::Id(0), &ports)
        .await
        .expect("reset should succeed");

    assert_eq!(name, "api");
    let record = table.find(&AppSelector::Id(0)).expect("record present");
    assert_eq!(record.runtime.restart_time, 0);
    assert_eq!(record.runtime.unstable_restarts, 0);
}

#[tokio::test]
async fn resetting_an_errored_app_marks_it_stopped() {
    let ports = FakePorts::new(1000);
    let mut table = started_table(&ports).await;
    let errored = table.find_mut(&AppSelector::Id(0)).expect("record present");
    errored.runtime.mark_exited(ProcessStatus::Errored);

    reset_app(&mut table, &AppSelector::Id(0), &ports)
        .await
        .expect("reset should succeed");

    let record = table.find(&AppSelector::Id(0)).expect("record present");
    assert_eq!(record.runtime.status, ProcessStatus::Stopped);
}

#[tokio::test]
async fn resetting_an_unknown_selector_reports_not_found() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    let err = reset_app(&mut table, &AppSelector::Id(9), &ports)
        .await
        .unwrap_err();
    assert!(matches!(err, UsecaseError::NotFound(_)), "got: {err}");
}

#[tokio::test]
async fn resetting_persists_the_table() {
    let ports = FakePorts::new(1000);
    let mut table = started_table(&ports).await;
    let saves_after_start = ports.save_count();
    reset_app(&mut table, &AppSelector::Id(0), &ports)
        .await
        .expect("reset should succeed");
    assert_eq!(ports.save_count(), saves_after_start + 1);
}

#[tokio::test]
async fn a_persistence_failure_propagates() {
    let ports = FakePorts::new(1000);
    let mut table = started_table(&ports).await;
    ports.fail_save();
    let err = reset_app(&mut table, &AppSelector::Id(0), &ports)
        .await
        .unwrap_err();
    assert!(matches!(err, UsecaseError::Dump(_)), "got: {err}");
}
