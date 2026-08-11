use entities::{AppSpec, ProcessStatus, ReadScope, SandboxMode, SandboxPolicy};

use super::*;
use crate::{
    AppSelector,
    fingerprint::render_identity,
    ports::Fingerprinter as _,
    ports_test_helpers::{FakePorts, LOGS_DIR, SANDBOX_PREFIX, live_token, spec, spec_with_deps},
};

async fn started(ports: &FakePorts) -> ProcessTable {
    let mut table = ProcessTable::new();
    start_apps(&mut table, &[spec("api")], LOGS_DIR, ports).await;
    table
}

fn recorded_identity(table: &ProcessTable) -> entities::ProcessIdentity {
    table
        .find(&AppSelector::Name("api".to_string()))
        .expect("record present")
        .runtime
        .identity
        .clone()
        .expect("a launched service carries an identity")
}

fn has_identity(table: &ProcessTable) -> bool {
    table
        .find(&AppSelector::Name("api".to_string()))
        .expect("record present")
        .runtime
        .identity
        .is_some()
}

fn failure(report: StartReport) -> UsecaseError {
    report
        .failure
        .expect("the report should carry the failure that stopped the batch")
}

#[tokio::test]
async fn starting_one_app_marks_it_online_with_a_pid() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    let report = start_apps(&mut table, &[spec("api")], LOGS_DIR, &ports).await;
    assert_eq!(report.outcomes.len(), 1);
    assert_eq!(report.outcomes[0].pid, Some(100));
    assert_eq!(report.outcomes[0].kind, StartKind::Spawned);
    let record = table
        .find(&AppSelector::Name("api".to_string()))
        .expect("record present");
    assert_eq!(record.runtime.status, ProcessStatus::Online);
}

#[tokio::test]
async fn dependencies_start_before_their_dependents() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    let specs = [spec_with_deps("web", &["api"]), spec("api")];
    start_apps(&mut table, &specs, LOGS_DIR, &ports).await;
    assert_eq!(ports.spawned_names(), vec!["api", "web"]);
}

#[tokio::test]
async fn a_dependency_already_in_the_table_satisfies_an_incremental_start() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    start_apps(&mut table, &[spec("api")], LOGS_DIR, &ports).await;
    start_apps(
        &mut table,
        &[spec_with_deps("web", &["api"])],
        LOGS_DIR,
        &ports,
    )
    .await;
    assert_eq!(ports.spawned_names(), vec!["api", "web"]);
}

#[tokio::test]
async fn a_dependency_missing_everywhere_still_blocks_the_start() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    let report = start_apps(
        &mut table,
        &[spec_with_deps("web", &["ghost"])],
        LOGS_DIR,
        &ports,
    )
    .await;
    let err = failure(report);
    assert!(matches!(err, UsecaseError::Dependency(_)), "got: {err}");
    assert!(ports.spawned_names().is_empty());
}

#[tokio::test]
async fn an_incremental_start_only_starts_the_apps_it_was_given() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    table.upsert(spec("api"), 1000);
    let report = start_apps(
        &mut table,
        &[spec_with_deps("web", &["api"])],
        LOGS_DIR,
        &ports,
    )
    .await;
    assert_eq!(report.outcomes.len(), 1);
    assert_eq!(ports.spawned_names(), vec!["web"]);
}

#[tokio::test]
async fn a_dependency_cycle_is_rejected_before_spawning_anything() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    let specs = [spec_with_deps("a", &["b"]), spec_with_deps("b", &["a"])];
    let err = failure(start_apps(&mut table, &specs, LOGS_DIR, &ports).await);
    assert!(matches!(err, UsecaseError::Dependency(_)), "got: {err}");
    assert!(ports.spawned_names().is_empty());
}

#[tokio::test]
async fn an_invalid_spec_is_rejected_before_spawning_anything() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    let broken = AppSpec {
        script: String::new(),
        ..spec("api")
    };
    let err = failure(start_apps(&mut table, &[broken], LOGS_DIR, &ports).await);
    assert!(matches!(err, UsecaseError::Spec(_)), "got: {err}");
    assert!(ports.spawned_names().is_empty());
}

#[tokio::test]
async fn the_launch_command_is_wrapped_by_the_sandbox() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    start_apps(&mut table, &[spec("api")], LOGS_DIR, &ports).await;
    let launched = ports.spawned();
    let launch = launched.first().expect("one spawn recorded");
    assert_eq!(launch.program, SANDBOX_PREFIX);
    assert_eq!(launch.args, ["/usr/bin/true".to_string()]);
}

#[tokio::test]
async fn launch_paths_point_at_the_app_log_files() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    start_apps(&mut table, &[spec("api")], LOGS_DIR, &ports).await;
    let launched = ports.spawned();
    let launch = launched.first().expect("one spawn recorded");
    assert_eq!(launch.stdout_path, "/fake/logs/api-out.log");
    assert_eq!(launch.stderr_path, "/fake/logs/api-err.log");
    assert_eq!(launch.cwd, "/srv/app");
}

#[tokio::test]
async fn a_sandbox_without_a_backend_blocks_the_start() {
    let ports = FakePorts::new(1000);
    ports.fail_wrap_for("api");
    let mut table = ProcessTable::new();
    let report = start_apps(&mut table, &[spec("api")], LOGS_DIR, &ports).await;
    let err = failure(report);
    assert!(matches!(err, UsecaseError::Sandbox(_)), "got: {err}");
    assert!(ports.spawned_names().is_empty());
}

#[tokio::test]
async fn a_spawn_failure_lands_in_the_report() {
    let ports = FakePorts::new(1000);
    ports.fail_spawn_for("api");
    let mut table = ProcessTable::new();
    let report = start_apps(&mut table, &[spec("api")], LOGS_DIR, &ports).await;
    let err = failure(report);
    assert!(matches!(err, UsecaseError::Launch(_)), "got: {err}");
}

#[tokio::test]
async fn a_later_failure_keeps_the_services_that_already_started() {
    let ports = FakePorts::new(1000);
    ports.fail_spawn_for("web");
    let mut table = ProcessTable::new();
    let specs = [spec_with_deps("web", &["api"]), spec("api")];
    let report = start_apps(&mut table, &specs, LOGS_DIR, &ports).await;
    assert_eq!(report.outcomes.len(), 1);
    assert_eq!(report.outcomes[0].name, "api");
    assert!(report.failure.is_some());
}

#[tokio::test]
async fn a_later_failure_still_persists_the_services_that_already_started() {
    let ports = FakePorts::new(1000);
    ports.fail_spawn_for("web");
    let mut table = ProcessTable::new();
    let specs = [spec_with_deps("web", &["api"]), spec("api")];
    start_apps(&mut table, &specs, LOGS_DIR, &ports).await;
    assert_eq!(ports.save_count(), 1);
}

#[tokio::test]
async fn a_failed_launch_leaves_no_record_of_the_unstarted_service() {
    let ports = FakePorts::new(1000);
    ports.fail_spawn_for("web");
    let mut table = ProcessTable::new();
    let specs = [spec_with_deps("web", &["api"]), spec("api")];
    start_apps(&mut table, &specs, LOGS_DIR, &ports).await;
    assert!(table.find(&AppSelector::Name("web".to_string())).is_none());
}

#[tokio::test]
async fn a_failed_launch_persists_only_the_services_that_started() {
    let ports = FakePorts::new(1000);
    ports.fail_spawn_for("web");
    let mut table = ProcessTable::new();
    let specs = [spec_with_deps("web", &["api"]), spec("api")];
    start_apps(&mut table, &specs, LOGS_DIR, &ports).await;
    let stored_names: Vec<String> = ports
        .stored()
        .iter()
        .map(|record| record.runtime.name.clone())
        .collect();
    assert_eq!(stored_names, vec!["api".to_string()]);
}

#[tokio::test]
async fn a_service_skipped_by_an_early_batch_failure_leaves_no_record() {
    let ports = FakePorts::new(1000);
    ports.fail_spawn_for("api");
    let mut table = ProcessTable::new();
    let specs = [spec_with_deps("web", &["api"]), spec("api")];
    start_apps(&mut table, &specs, LOGS_DIR, &ports).await;
    assert!(table.records().is_empty());
    assert!(ports.stored().is_empty());
}

#[tokio::test]
async fn a_preexisting_record_survives_a_failed_relaunch() {
    let ports = FakePorts::new(1000);
    ports.fail_spawn_for("web");
    let mut table = ProcessTable::new();
    table.upsert(spec("web"), 1000);
    start_apps(&mut table, &[spec("web")], LOGS_DIR, &ports).await;
    assert!(table.find(&AppSelector::Name("web".to_string())).is_some());
}

#[tokio::test]
async fn starting_an_already_running_app_is_idempotent() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    start_apps(&mut table, &[spec("api")], LOGS_DIR, &ports).await;
    let report = start_apps(&mut table, &[spec("api")], LOGS_DIR, &ports).await;
    assert_eq!(report.outcomes[0].kind, StartKind::AlreadyRunning);
    assert_eq!(report.outcomes[0].pid, Some(100));
    assert_eq!(ports.spawned_names().len(), 1);
}

#[tokio::test]
async fn a_successful_start_persists_the_table() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    start_apps(&mut table, &[spec("api")], LOGS_DIR, &ports).await;
    assert_eq!(ports.save_count(), 1);
    assert_eq!(ports.stored().len(), 1);
}

#[path = "start_identity_tests.rs"]
mod identity;
