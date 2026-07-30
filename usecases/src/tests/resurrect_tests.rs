use entities::{ProcessRuntime, ProcessStatus};

use super::*;
use crate::{
    AppSelector, UsecaseError,
    ports_test_helpers::{FakePorts, LOGS_DIR, spec, spec_with_deps},
};

fn stored_record(name: &str, pm_id: u32, status: ProcessStatus) -> ProcessRecord {
    let mut runtime = ProcessRuntime::new(pm_id, name.to_string(), 1000);
    runtime.mark_launched(7, 1000);
    runtime.status = status;
    ProcessRecord {
        spec: spec(name),
        runtime,
    }
}

#[tokio::test]
async fn an_empty_state_file_revives_nothing() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    let outcomes = resurrect(&mut table, LOGS_DIR, &ports)
        .await
        .expect("resurrect should succeed");
    assert!(outcomes.is_empty());
    assert!(table.records().is_empty());
}

#[tokio::test]
async fn apps_that_were_online_are_started_again() {
    let ports = FakePorts::new(1000);
    ports.seed_stored(vec![stored_record("api", 0, ProcessStatus::Online)]);
    let mut table = ProcessTable::new();
    let outcomes = resurrect(&mut table, LOGS_DIR, &ports)
        .await
        .expect("resurrect should succeed");
    assert_eq!(outcomes.len(), 1);
    assert_eq!(ports.spawned_names(), vec!["api"]);
    let record = table.find(&AppSelector::Id(0)).expect("record present");
    assert_eq!(record.runtime.status, ProcessStatus::Online);
}

#[tokio::test]
async fn apps_caught_mid_shutdown_are_started_again() {
    let ports = FakePorts::new(1000);
    ports.seed_stored(vec![stored_record("api", 0, ProcessStatus::Stopping)]);
    let mut table = ProcessTable::new();
    resurrect(&mut table, LOGS_DIR, &ports)
        .await
        .expect("resurrect should succeed");
    assert_eq!(ports.spawned_names(), vec!["api"]);
}

#[tokio::test]
async fn apps_that_were_stopped_stay_stopped_but_remain_known() {
    let ports = FakePorts::new(1000);
    ports.seed_stored(vec![stored_record("api", 0, ProcessStatus::Stopped)]);
    let mut table = ProcessTable::new();
    let outcomes = resurrect(&mut table, LOGS_DIR, &ports)
        .await
        .expect("resurrect should succeed");
    assert!(outcomes.is_empty());
    assert!(ports.spawned_names().is_empty());
    assert_eq!(table.records().len(), 1);
}

#[tokio::test]
async fn errored_apps_are_not_revived() {
    let ports = FakePorts::new(1000);
    ports.seed_stored(vec![stored_record("api", 0, ProcessStatus::Errored)]);
    let mut table = ProcessTable::new();
    resurrect(&mut table, LOGS_DIR, &ports)
        .await
        .expect("resurrect should succeed");
    assert!(ports.spawned_names().is_empty());
}

#[tokio::test]
async fn revived_apps_follow_their_dependency_order() {
    let ports = FakePorts::new(1000);
    let mut web = stored_record("web", 1, ProcessStatus::Online);
    web.spec = spec_with_deps("web", &["api"]);
    ports.seed_stored(vec![web, stored_record("api", 0, ProcessStatus::Online)]);
    let mut table = ProcessTable::new();
    resurrect(&mut table, LOGS_DIR, &ports)
        .await
        .expect("resurrect should succeed");
    assert_eq!(ports.spawned_names(), vec!["api", "web"]);
}

#[tokio::test]
async fn a_stale_pid_is_discarded_before_restarting() {
    let ports = FakePorts::new(1000);
    ports.seed_stored(vec![stored_record("api", 0, ProcessStatus::Online)]);
    let mut table = ProcessTable::new();
    resurrect(&mut table, LOGS_DIR, &ports)
        .await
        .expect("resurrect should succeed");
    let record = table.find(&AppSelector::Id(0)).expect("record present");
    assert_eq!(record.runtime.pid, Some(100));
}

#[tokio::test]
async fn a_pending_restart_flag_does_not_survive_a_daemon_restart() {
    let ports = FakePorts::new(1000);
    let mut record = stored_record("api", 0, ProcessStatus::Stopped);
    record.runtime.request_restart();
    ports.seed_stored(vec![record]);
    let mut table = ProcessTable::new();
    resurrect(&mut table, LOGS_DIR, &ports)
        .await
        .expect("resurrect should succeed");
    let stored = table.find(&AppSelector::Id(0)).expect("record present");
    assert!(!stored.runtime.pending_restart);
}

#[tokio::test]
async fn a_read_failure_propagates() {
    let ports = FakePorts::new(1000);
    ports.fail_load();
    let mut table = ProcessTable::new();
    let err = resurrect(&mut table, LOGS_DIR, &ports).await.unwrap_err();
    assert!(matches!(err, UsecaseError::Dump(_)), "got: {err}");
}

#[tokio::test]
async fn a_persistence_failure_propagates() {
    let ports = FakePorts::new(1000);
    ports.seed_stored(vec![stored_record("api", 0, ProcessStatus::Stopped)]);
    ports.fail_save();
    let mut table = ProcessTable::new();
    let err = resurrect(&mut table, LOGS_DIR, &ports).await.unwrap_err();
    assert!(matches!(err, UsecaseError::Dump(_)), "got: {err}");
}

#[tokio::test]
async fn a_failure_to_revive_an_app_propagates() {
    let ports = FakePorts::new(1000);
    ports.seed_stored(vec![stored_record("api", 0, ProcessStatus::Online)]);
    ports.fail_spawn_for("api");
    let mut table = ProcessTable::new();
    let err = resurrect(&mut table, LOGS_DIR, &ports).await.unwrap_err();
    assert!(matches!(err, UsecaseError::Launch(_)), "got: {err}");
}

#[tokio::test]
async fn a_cyclic_state_file_is_rejected() {
    let ports = FakePorts::new(1000);
    let mut first = stored_record("a", 0, ProcessStatus::Stopped);
    first.spec = spec_with_deps("a", &["b"]);
    let mut second = stored_record("b", 1, ProcessStatus::Stopped);
    second.spec = spec_with_deps("b", &["a"]);
    ports.seed_stored(vec![first, second]);
    let mut table = ProcessTable::new();
    let err = resurrect(&mut table, LOGS_DIR, &ports).await.unwrap_err();
    assert!(matches!(err, UsecaseError::Dependency(_)), "got: {err}");
}
