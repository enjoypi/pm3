use entities::{ProcessIdentity, ProcessRuntime, ProcessStatus};

use super::*;
use crate::{
    AppSelector, UsecaseError,
    fingerprint::render_identity,
    ports::Fingerprinter as _,
    ports_test_helpers::{FakePorts, LOGS_DIR, live_token, spec, spec_with_deps},
};

const SURVIVOR_PID: u32 = 7;
const KILL_TIMEOUT_MS: u64 = 1600;

fn stored_record(name: &str, pm_id: u32, status: ProcessStatus) -> ProcessRecord {
    let mut runtime = ProcessRuntime::new(pm_id, name.to_string(), 1000);
    runtime.mark_launched(SURVIVOR_PID, 1000);
    runtime.status = status;
    ProcessRecord {
        spec: spec(name),
        runtime,
    }
}

fn expected_identity(ports: &FakePorts, record: &ProcessRecord) -> ProcessIdentity {
    ProcessIdentity {
        token: live_token(SURVIVOR_PID),
        launch_digest: ports.digest(&render_identity(&record.spec)),
        binary_digest: format!("file:{}", record.spec.script),
    }
}

fn survivor(ports: &FakePorts, name: &str) -> ProcessRecord {
    let mut record = stored_record(name, 0, ProcessStatus::Online);
    record.runtime.identity = Some(expected_identity(ports, &record));
    ports.seed_live(SURVIVOR_PID, &live_token(SURVIVOR_PID));
    record
}

async fn resurrected(ports: &FakePorts) -> ProcessTable {
    let mut table = ProcessTable::new();
    resurrect(&mut table, LOGS_DIR, KILL_TIMEOUT_MS, ports)
        .await
        .expect("resurrect should succeed");
    table
}

fn revived_pid(table: &ProcessTable) -> u32 {
    table
        .find(&AppSelector::Id(0))
        .expect("record present")
        .runtime
        .pid
        .expect("a revived service holds a pid")
}

fn probing_survivor(ports: &FakePorts, name: &str) -> ProcessRecord {
    let mut record = stored_record(name, 0, ProcessStatus::Launching);
    record.spec = crate::ports_test_helpers::spec_probed(name);
    record.runtime.identity = Some(expected_identity(ports, &record));
    ports.seed_live(SURVIVOR_PID, &live_token(SURVIVOR_PID));
    record
}

#[tokio::test]
async fn a_survivor_that_was_still_probing_keeps_waiting_for_readiness() {
    let ports = FakePorts::new(1000);
    ports.seed_stored(vec![probing_survivor(&ports, "api")]);
    let table = resurrected(&ports).await;
    let record = table.find(&AppSelector::Id(0)).expect("record present");
    assert_eq!(record.runtime.status, ProcessStatus::Launching);
    assert_eq!(record.runtime.pid, Some(SURVIVOR_PID));
}

#[tokio::test]
async fn a_probing_record_without_a_probe_adopts_as_online() {
    let ports = FakePorts::new(1000);
    let mut record = survivor(&ports, "api");
    record.runtime.status = ProcessStatus::Launching;
    ports.seed_stored(vec![record]);
    let table = resurrected(&ports).await;
    let stored = table.find(&AppSelector::Id(0)).expect("record present");
    assert_eq!(stored.runtime.status, ProcessStatus::Online);
}

#[tokio::test]
async fn an_empty_state_file_revives_nothing() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    let outcomes = resurrect(&mut table, LOGS_DIR, KILL_TIMEOUT_MS, &ports)
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
    let outcomes = resurrect(&mut table, LOGS_DIR, KILL_TIMEOUT_MS, &ports)
        .await
        .expect("resurrect should succeed");
    assert_eq!(outcomes.len(), 1);
    assert_eq!(ports.spawned_names(), vec!["api"]);
    let record = table.find(&AppSelector::Id(0)).expect("record present");
    assert_eq!(record.runtime.status, ProcessStatus::Online);
}

#[tokio::test]
async fn apps_caught_mid_shutdown_are_settled_rather_than_started_again() {
    let ports = FakePorts::new(1000);
    ports.seed_stored(vec![stored_record("api", 0, ProcessStatus::Stopping)]);
    let mut table = ProcessTable::new();
    resurrect(&mut table, LOGS_DIR, KILL_TIMEOUT_MS, &ports)
        .await
        .expect("resurrect should succeed");
    assert!(ports.spawned_names().is_empty());
}

#[tokio::test]
async fn an_app_caught_mid_shutdown_stays_known_as_stopped() {
    let ports = FakePorts::new(1000);
    ports.seed_stored(vec![stored_record("api", 0, ProcessStatus::Stopping)]);
    let mut table = ProcessTable::new();
    resurrect(&mut table, LOGS_DIR, KILL_TIMEOUT_MS, &ports)
        .await
        .expect("resurrect should succeed");
    let record = table.find(&AppSelector::Id(0)).expect("record present");
    assert_eq!(record.runtime.status, ProcessStatus::Stopped);
}

#[tokio::test]
async fn apps_that_were_stopped_stay_stopped_but_remain_known() {
    let ports = FakePorts::new(1000);
    ports.seed_stored(vec![stored_record("api", 0, ProcessStatus::Stopped)]);
    let mut table = ProcessTable::new();
    let outcomes = resurrect(&mut table, LOGS_DIR, KILL_TIMEOUT_MS, &ports)
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
    resurrect(&mut table, LOGS_DIR, KILL_TIMEOUT_MS, &ports)
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
    resurrect(&mut table, LOGS_DIR, KILL_TIMEOUT_MS, &ports)
        .await
        .expect("resurrect should succeed");
    assert_eq!(ports.spawned_names(), vec!["api", "web"]);
}

#[tokio::test]
async fn a_stale_pid_is_discarded_before_restarting() {
    let ports = FakePorts::new(1000);
    ports.seed_stored(vec![stored_record("api", 0, ProcessStatus::Online)]);
    let mut table = ProcessTable::new();
    resurrect(&mut table, LOGS_DIR, KILL_TIMEOUT_MS, &ports)
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
    resurrect(&mut table, LOGS_DIR, KILL_TIMEOUT_MS, &ports)
        .await
        .expect("resurrect should succeed");
    let stored = table.find(&AppSelector::Id(0)).expect("record present");
    assert!(!stored.runtime.pending_restart);
}

#[tokio::test]
async fn a_restart_interrupted_mid_drain_is_finished_by_the_next_daemon() {
    let ports = FakePorts::new(1000);
    let mut record = survivor(&ports, "api");
    record.runtime.status = ProcessStatus::Stopping;
    record.runtime.request_restart();
    ports.seed_stored(vec![record]);
    let table = resurrected(&ports).await;
    assert_eq!(ports.terminated(), vec![SURVIVOR_PID]);
    assert_eq!(ports.spawned_names(), vec!["api"]);
    let stored = table.find(&AppSelector::Id(0)).expect("record present");
    assert_eq!(stored.runtime.status, ProcessStatus::Online);
    assert!(!stored.runtime.pending_restart);
}

#[tokio::test]
async fn a_read_failure_propagates() {
    let ports = FakePorts::new(1000);
    ports.fail_load();
    let mut table = ProcessTable::new();
    let err = resurrect(&mut table, LOGS_DIR, KILL_TIMEOUT_MS, &ports)
        .await
        .unwrap_err();
    assert!(matches!(err, UsecaseError::Dump(_)), "got: {err}");
}

#[tokio::test]
async fn an_untouched_survivor_is_reclaimed_instead_of_restarted() {
    let ports = FakePorts::new(1000);
    ports.seed_stored(vec![survivor(&ports, "api")]);
    let outcomes = resurrect(&mut ProcessTable::new(), LOGS_DIR, KILL_TIMEOUT_MS, &ports)
        .await
        .expect("resurrect should succeed");
    assert_eq!(outcomes[0].kind, StartKind::Adopted);
    assert!(ports.spawned_names().is_empty(), "nothing should respawn");
}

#[tokio::test]
async fn a_reclaimed_survivor_keeps_the_pid_it_was_already_running_under() {
    let ports = FakePorts::new(1000);
    ports.seed_stored(vec![survivor(&ports, "api")]);
    let table = resurrected(&ports).await;
    assert_eq!(revived_pid(&table), SURVIVOR_PID);
}

#[tokio::test]
async fn a_reclaimed_survivor_is_handed_to_the_launcher_for_tracking() {
    let ports = FakePorts::new(1000);
    ports.seed_stored(vec![survivor(&ports, "api")]);
    resurrected(&ports).await;
    assert_eq!(ports.adopted(), vec![SURVIVOR_PID]);
}

#[tokio::test]
async fn a_reclaimed_survivor_stays_online() {
    let ports = FakePorts::new(1000);
    ports.seed_stored(vec![survivor(&ports, "api")]);
    let table = resurrected(&ports).await;
    let record = table.find(&AppSelector::Id(0)).expect("record present");
    assert_eq!(record.runtime.status, ProcessStatus::Online);
}

#[tokio::test]
async fn a_survivor_caught_mid_shutdown_is_settled_instead_of_revived() {
    let ports = FakePorts::new(1000);
    let mut record = survivor(&ports, "api");
    record.runtime.status = ProcessStatus::Stopping;
    ports.seed_stored(vec![record]);
    let table = resurrected(&ports).await;
    let stored = table.find(&AppSelector::Id(0)).expect("record present");
    assert_eq!(stored.runtime.status, ProcessStatus::Stopped);
}

#[tokio::test]
async fn a_survivor_caught_mid_shutdown_is_terminated_by_the_next_daemon() {
    let ports = FakePorts::new(1000);
    let mut record = survivor(&ports, "api");
    record.runtime.status = ProcessStatus::Stopping;
    ports.seed_stored(vec![record]);
    resurrected(&ports).await;
    assert_eq!(ports.terminated(), vec![SURVIVOR_PID]);
}

#[tokio::test]
async fn a_pid_that_already_left_mid_shutdown_is_not_signalled_again() {
    let ports = FakePorts::new(1000);
    let mut record = survivor(&ports, "api");
    record.runtime.status = ProcessStatus::Stopping;
    ports.seed_stored(vec![record]);
    ports.hide_from_probe(SURVIVOR_PID);
    resurrected(&ports).await;
    assert!(ports.terminated().is_empty());
}

#[tokio::test]
async fn a_pid_the_kernel_reused_mid_shutdown_is_spared() {
    let ports = FakePorts::new(1000);
    let mut record = survivor(&ports, "api");
    record.runtime.status = ProcessStatus::Stopping;
    ports.seed_stored(vec![record]);
    ports.seed_live(SURVIVOR_PID, "some other process");
    resurrected(&ports).await;
    assert!(ports.terminated().is_empty());
}

#[tokio::test]
async fn a_pid_pm3_cannot_probe_mid_shutdown_is_stopped_anyway() {
    let ports = FakePorts::new(1000);
    let mut record = survivor(&ports, "api");
    record.runtime.status = ProcessStatus::Stopping;
    ports.seed_stored(vec![record]);
    ports.break_probe_for(SURVIVOR_PID);
    resurrected(&ports).await;
    assert_eq!(ports.terminated(), vec![SURVIVOR_PID]);
}

#[tokio::test]
async fn a_record_without_a_pid_mid_shutdown_signals_nothing() {
    let ports = FakePorts::new(1000);
    let mut record = survivor(&ports, "api");
    record.runtime.status = ProcessStatus::Stopping;
    record.runtime.pid = None;
    ports.seed_stored(vec![record]);
    resurrected(&ports).await;
    assert!(ports.terminated().is_empty());
}

#[tokio::test]
async fn a_survivor_caught_mid_shutdown_is_not_started_again() {
    let ports = FakePorts::new(1000);
    let mut record = survivor(&ports, "api");
    record.runtime.status = ProcessStatus::Stopping;
    ports.seed_stored(vec![record]);
    resurrected(&ports).await;
    assert!(ports.spawned_names().is_empty());
}

#[tokio::test]
async fn a_service_that_left_while_the_daemon_was_down_is_restarted() {
    let ports = FakePorts::new(1000);
    ports.seed_stored(vec![survivor(&ports, "api")]);
    ports.kill_silently(SURVIVOR_PID);
    let table = resurrected(&ports).await;
    assert_eq!(ports.spawned_names(), vec!["api"]);
    assert_eq!(revived_pid(&table), 100);
}

#[tokio::test]
async fn a_pid_the_system_handed_to_someone_else_is_not_reclaimed() {
    let ports = FakePorts::new(1000);
    ports.seed_stored(vec![survivor(&ports, "api")]);
    ports.seed_live(SURVIVOR_PID, "Wed Jul 29 09:00:00 2026");
    resurrected(&ports).await;
    assert_eq!(ports.spawned_names(), vec!["api"]);
}

#[tokio::test]
async fn a_service_whose_launch_arguments_changed_is_restarted() {
    let ports = FakePorts::new(1000);
    let mut record = survivor(&ports, "api");
    record.runtime.identity = Some(ProcessIdentity {
        launch_digest: "stale".to_string(),
        ..expected_identity(&ports, &record)
    });
    ports.seed_stored(vec![record]);
    resurrected(&ports).await;
    assert_eq!(ports.spawned_names(), vec!["api"]);
}

#[tokio::test]
async fn a_service_whose_program_was_replaced_is_restarted() {
    let ports = FakePorts::new(1000);
    let mut record = survivor(&ports, "api");
    record.runtime.identity = Some(ProcessIdentity {
        binary_digest: "stale".to_string(),
        ..expected_identity(&ports, &record)
    });
    ports.seed_stored(vec![record]);
    resurrected(&ports).await;
    assert_eq!(ports.spawned_names(), vec!["api"]);
}

#[path = "resurrect_boot_tests.rs"]
mod boot;

#[path = "resurrect_evict_tests.rs"]
mod evict;

#[path = "resurrect_stranded_tests.rs"]
mod stranded;
