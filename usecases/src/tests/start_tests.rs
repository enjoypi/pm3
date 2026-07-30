use entities::{ProcessStatus, SandboxMode, SandboxPolicy};

use super::*;
use crate::{
    AppSelector,
    ports_test_helpers::{FakePorts, LOGS_DIR, SANDBOX_PREFIX, spec, spec_with_deps},
};

#[tokio::test]
async fn starting_one_app_marks_it_online_with_a_pid() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    let outcomes = start_apps(&mut table, &[spec("api")], LOGS_DIR, &ports)
        .await
        .expect("start should succeed");
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].pid, Some(100));
    assert!(!outcomes[0].already_running);
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
    start_apps(&mut table, &specs, LOGS_DIR, &ports)
        .await
        .expect("start should succeed");
    assert_eq!(ports.spawned_names(), vec!["api", "web"]);
}

#[tokio::test]
async fn a_dependency_cycle_is_rejected_before_spawning_anything() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    let specs = [spec_with_deps("a", &["b"]), spec_with_deps("b", &["a"])];
    let err = start_apps(&mut table, &specs, LOGS_DIR, &ports)
        .await
        .unwrap_err();
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
    let err = start_apps(&mut table, &[broken], LOGS_DIR, &ports)
        .await
        .unwrap_err();
    assert!(matches!(err, UsecaseError::Spec(_)), "got: {err}");
    assert!(ports.spawned_names().is_empty());
}

#[tokio::test]
async fn the_launch_command_is_wrapped_by_the_sandbox() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    start_apps(&mut table, &[spec("api")], LOGS_DIR, &ports)
        .await
        .expect("start should succeed");
    let launched = ports.spawned();
    let launch = launched.first().expect("one spawn recorded");
    assert_eq!(launch.program, SANDBOX_PREFIX);
    assert_eq!(launch.args, ["/usr/bin/true".to_string()]);
}

#[tokio::test]
async fn launch_paths_point_at_the_app_log_files() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    start_apps(&mut table, &[spec("api")], LOGS_DIR, &ports)
        .await
        .expect("start should succeed");
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
    let err = start_apps(&mut table, &[spec("api")], LOGS_DIR, &ports)
        .await
        .unwrap_err();
    assert!(matches!(err, UsecaseError::Sandbox(_)), "got: {err}");
    assert!(ports.spawned_names().is_empty());
}

#[tokio::test]
async fn a_spawn_failure_propagates() {
    let ports = FakePorts::new(1000);
    ports.fail_spawn_for("api");
    let mut table = ProcessTable::new();
    let err = start_apps(&mut table, &[spec("api")], LOGS_DIR, &ports)
        .await
        .unwrap_err();
    assert!(matches!(err, UsecaseError::Launch(_)), "got: {err}");
}

#[tokio::test]
async fn starting_an_already_running_app_is_idempotent() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    start_apps(&mut table, &[spec("api")], LOGS_DIR, &ports)
        .await
        .expect("first start");
    let outcomes = start_apps(&mut table, &[spec("api")], LOGS_DIR, &ports)
        .await
        .expect("second start");
    assert!(outcomes[0].already_running);
    assert_eq!(outcomes[0].pid, Some(100));
    assert_eq!(ports.spawned_names().len(), 1);
}

#[tokio::test]
async fn a_successful_start_persists_the_table() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    start_apps(&mut table, &[spec("api")], LOGS_DIR, &ports)
        .await
        .expect("start should succeed");
    assert_eq!(ports.save_count(), 1);
    assert_eq!(ports.stored().len(), 1);
}

#[tokio::test]
async fn a_persistence_failure_propagates() {
    let ports = FakePorts::new(1000);
    ports.fail_save();
    let mut table = ProcessTable::new();
    let err = start_apps(&mut table, &[spec("api")], LOGS_DIR, &ports)
        .await
        .unwrap_err();
    assert!(matches!(err, UsecaseError::Dump(_)), "got: {err}");
}

#[tokio::test]
async fn an_unconfined_app_runs_without_a_sandbox_wrapper() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    let unconfined = AppSpec {
        sandbox: SandboxPolicy {
            mode: SandboxMode::DangerFullAccess,
            network: true,
            writable_roots: Vec::new(),
        },
        ..spec("api")
    };
    start_apps(&mut table, &[unconfined], LOGS_DIR, &ports)
        .await
        .expect("start should succeed");
    let launched = ports.spawned();
    let launch = launched.first().expect("one spawn recorded");
    assert_eq!(launch.program, "/usr/bin/true");
    assert!(launch.args.is_empty());
}

#[tokio::test]
async fn starting_an_unknown_name_reports_not_found() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    let err = start_one(&mut table, "ghost", LOGS_DIR, &ports)
        .await
        .unwrap_err();
    assert!(matches!(err, UsecaseError::NotFound(_)), "got: {err}");
}
