use entities::{ProcessIdentity, ProcessRuntime, ProcessStatus};

use super::*;
use crate::{
    AppSelector, UsecaseError,
    ports::Fingerprinter as _,
    ports_test_helpers::{FakePorts, LOGS_DIR, live_token, spec, spec_with_deps},
};

const SURVIVOR_PID: u32 = 7;

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
    resurrect(&mut table, LOGS_DIR, ports)
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
async fn an_untouched_survivor_is_reclaimed_instead_of_restarted() {
    let ports = FakePorts::new(1000);
    ports.seed_stored(vec![survivor(&ports, "api")]);
    let outcomes = resurrect(&mut ProcessTable::new(), LOGS_DIR, &ports)
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
async fn a_survivor_caught_mid_shutdown_is_reclaimed_and_promoted_back_online() {
    let ports = FakePorts::new(1000);
    let mut record = survivor(&ports, "api");
    record.runtime.status = ProcessStatus::Stopping;
    ports.seed_stored(vec![record]);
    let table = resurrected(&ports).await;
    let stored = table.find(&AppSelector::Id(0)).expect("record present");
    assert_eq!(stored.runtime.status, ProcessStatus::Online);
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

#[tokio::test]
async fn a_stale_survivor_is_stopped_before_its_replacement_starts() {
    let ports = FakePorts::new(1000);
    let mut record = survivor(&ports, "api");
    record.runtime.identity = Some(ProcessIdentity {
        launch_digest: "stale".to_string(),
        ..expected_identity(&ports, &record)
    });
    ports.seed_stored(vec![record]);
    resurrected(&ports).await;
    assert_eq!(ports.terminated(), vec![SURVIVOR_PID]);
}

#[tokio::test]
async fn a_survivor_that_already_left_is_not_signalled_again() {
    let ports = FakePorts::new(1000);
    ports.seed_stored(vec![survivor(&ports, "api")]);
    ports.kill_silently(SURVIVOR_PID);
    resurrected(&ports).await;
    assert!(ports.terminated().is_empty());
}

#[tokio::test]
async fn a_stale_survivor_that_refuses_the_signal_still_gets_a_replacement() {
    let ports = FakePorts::new(1000);
    let mut record = survivor(&ports, "api");
    record.runtime.identity = Some(ProcessIdentity {
        binary_digest: "stale".to_string(),
        ..expected_identity(&ports, &record)
    });
    ports.seed_stored(vec![record]);
    ports.fail_signal_for(SURVIVOR_PID);
    resurrected(&ports).await;
    assert_eq!(ports.spawned_names(), vec!["api"]);
}

#[tokio::test]
async fn a_service_whose_program_cannot_be_digested_is_restarted() {
    let ports = FakePorts::new(1000);
    ports.seed_stored(vec![survivor(&ports, "api")]);
    ports.fail_file_digest_for("/usr/bin/true");
    resurrected(&ports).await;
    assert_eq!(ports.spawned_names(), vec!["api"]);
}

#[tokio::test]
async fn a_service_that_must_respawn_without_a_sandbox_fails_loudly() {
    let ports = FakePorts::new(1000);
    let mut record = survivor(&ports, "api");
    record.runtime.identity = None;
    ports.seed_stored(vec![record]);
    ports.fail_wrap_for("api");
    let err = resurrect(&mut ProcessTable::new(), LOGS_DIR, &ports)
        .await
        .unwrap_err();
    assert!(matches!(err, UsecaseError::Sandbox(_)), "got: {err}");
}

#[tokio::test]
async fn a_live_service_is_reclaimed_even_when_the_sandbox_can_no_longer_wrap() {
    let ports = FakePorts::new(1000);
    ports.seed_stored(vec![survivor(&ports, "api")]);
    ports.fail_wrap_for("api");
    let table = resurrected(&ports).await;
    let record = table
        .find(&AppSelector::Name("api".to_string()))
        .expect("record present");
    assert_eq!(record.runtime.pid, Some(SURVIVOR_PID));
    assert!(
        ports.spawned_names().is_empty(),
        "a reclaimed process needs no fresh wrapping"
    );
}

#[tokio::test]
async fn a_dump_written_before_identities_existed_restarts_everything() {
    let ports = FakePorts::new(1000);
    let mut record = survivor(&ports, "api");
    record.runtime.identity = None;
    ports.seed_stored(vec![record]);
    resurrected(&ports).await;
    assert_eq!(ports.spawned_names(), vec!["api"]);
}

#[tokio::test]
async fn reclaimed_and_restarted_services_can_be_mixed() {
    let ports = FakePorts::new(1000);
    let kept = survivor(&ports, "api");
    let mut lost = stored_record("web", 1, ProcessStatus::Online);
    lost.runtime.identity = None;
    ports.seed_stored(vec![kept, lost]);
    let outcomes = resurrect(&mut ProcessTable::new(), LOGS_DIR, &ports)
        .await
        .expect("resurrect should succeed");
    let kinds: Vec<(&str, StartKind)> = outcomes
        .iter()
        .map(|outcome| (outcome.name.as_str(), outcome.kind))
        .collect();
    assert_eq!(
        kinds,
        vec![("api", StartKind::Adopted), ("web", StartKind::Spawned)]
    );
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
