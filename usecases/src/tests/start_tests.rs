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

#[tokio::test]
async fn starting_a_service_that_is_still_stopping_queues_a_restart() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    start_apps(&mut table, &[spec("api")], LOGS_DIR, &ports).await;
    crate::stop_app(&mut table, &AppSelector::Name("api".to_string()), &ports)
        .await
        .expect("stop should succeed");

    start_apps(&mut table, &[spec("api")], LOGS_DIR, &ports).await;

    let record = table
        .find(&AppSelector::Name("api".to_string()))
        .expect("record present");
    assert!(
        record.runtime.pending_restart,
        "a start racing a stop must not leave the service settled and unwatched"
    );
}

#[tokio::test]
async fn starting_a_service_that_is_still_stopping_re_arms_its_schedule() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    start_apps(&mut table, &[spec("api")], LOGS_DIR, &ports).await;
    crate::stop_app(&mut table, &AppSelector::Name("api".to_string()), &ports)
        .await
        .expect("stop should succeed");

    start_apps(&mut table, &[spec("api")], LOGS_DIR, &ports).await;

    let record = table
        .find(&AppSelector::Name("api".to_string()))
        .expect("record present");
    assert!(record.runtime.schedule_armed);
}

#[tokio::test]
async fn a_persistence_failure_lands_in_its_own_field() {
    let ports = FakePorts::new(1000);
    ports.fail_save();
    let mut table = ProcessTable::new();
    let report = start_apps(&mut table, &[spec("api")], LOGS_DIR, &ports).await;
    let err = report
        .unsaved
        .expect("the report should carry the persistence failure");
    assert!(matches!(err, UsecaseError::Dump(_)), "got: {err}");
}

#[tokio::test]
async fn a_persistence_failure_does_not_pose_as_a_refused_service() {
    let ports = FakePorts::new(1000);
    ports.fail_save();
    let mut table = ProcessTable::new();
    let report = start_apps(&mut table, &[spec("api")], LOGS_DIR, &ports).await;
    assert!(report.failure.is_none(), "the service did start");
    assert_eq!(report.outcomes.len(), 1);
}

#[tokio::test]
async fn a_launch_failure_and_a_persistence_failure_are_reported_apart() {
    let ports = FakePorts::new(1000);
    ports.fail_spawn_for("api");
    ports.fail_save();
    let mut table = ProcessTable::new();
    let report = start_apps(&mut table, &[spec("api")], LOGS_DIR, &ports).await;
    let launch = report.failure.expect("the launch failure is kept");
    assert!(matches!(launch, UsecaseError::Launch(_)), "got: {launch}");
    let unsaved = report
        .unsaved
        .expect("the persistence failure is kept as well");
    assert!(matches!(unsaved, UsecaseError::Dump(_)), "got: {unsaved}");
}

#[tokio::test]
async fn an_unconfined_app_runs_without_a_sandbox_wrapper() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    let unconfined = AppSpec {
        sandbox: SandboxPolicy {
            mode: SandboxMode::DangerFullAccess,
            read: ReadScope::Minimal,
            network: true,
            writable_roots: Vec::new(),
            readable_roots: Vec::new(),
            derived_roots: Vec::new(),
            unreadable_roots: Vec::new(),
        },
        ..spec("api")
    };
    start_apps(&mut table, &[unconfined], LOGS_DIR, &ports).await;
    let launched = ports.spawned();
    let launch = launched.first().expect("one spawn recorded");
    assert_eq!(launch.program, "/usr/bin/true");
    assert!(launch.args.is_empty());
}

#[tokio::test]
async fn a_started_app_records_the_identity_of_the_process_it_launched() {
    let ports = FakePorts::new(1000);
    let table = started(&ports).await;
    let identity = recorded_identity(&table);
    assert_eq!(identity.token, live_token(100));
}

#[tokio::test]
async fn a_started_app_records_the_digest_of_what_it_was_launched_with() {
    let ports = FakePorts::new(1000);
    let table = started(&ports).await;
    let identity = recorded_identity(&table);
    assert_eq!(
        identity.launch_digest,
        ports.digest(&render_identity(&spec("api")))
    );
}

#[tokio::test]
async fn a_started_app_records_the_digest_of_its_program() {
    let ports = FakePorts::new(1000);
    ports.seed_file_digest("/usr/bin/true", "cafe");
    let table = started(&ports).await;
    let identity = recorded_identity(&table);
    assert_eq!(identity.binary_digest, "cafe");
}

#[tokio::test]
async fn a_process_that_vanished_before_it_could_be_probed_records_no_identity() {
    let ports = FakePorts::new(1000);
    ports.hide_from_probe(100);
    let table = started(&ports).await;
    assert!(!has_identity(&table));
}

#[tokio::test]
async fn an_undigestable_program_records_no_identity_but_still_starts() {
    let ports = FakePorts::new(1000);
    ports.fail_file_digest_for("/usr/bin/true");
    let table = started(&ports).await;
    let record = table
        .find(&AppSelector::Name("api".to_string()))
        .expect("record present");
    assert_eq!(record.runtime.status, ProcessStatus::Online);
    assert_eq!(record.runtime.identity, None);
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

#[tokio::test]
async fn a_scheduled_one_shot_app_is_registered_without_spawning() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    let task = AppSpec {
        autorestart: false,
        schedule: Some("* * * * *".to_string()),
        ..spec("sweep")
    };
    let report = start_apps(&mut table, &[task], LOGS_DIR, &ports).await;
    assert_eq!(report.outcomes[0].kind, StartKind::Scheduled);
    assert!(ports.spawned_names().is_empty());
    let record = table
        .find(&AppSelector::Name("sweep".to_string()))
        .expect("record present");
    assert_eq!(record.runtime.status, ProcessStatus::Stopped);
}

#[tokio::test]
async fn a_scheduled_app_that_autorestarts_still_spawns() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    let service = AppSpec {
        autorestart: true,
        schedule: Some("* * * * *".to_string()),
        ..spec("api")
    };
    let report = start_apps(&mut table, &[service], LOGS_DIR, &ports).await;
    assert_eq!(report.outcomes[0].kind, StartKind::Spawned);
}

#[test]
fn a_scheduled_registration_needs_no_watching() {
    assert!(!StartKind::Scheduled.needs_watching());
}

#[test]
fn only_settled_registrations_skip_the_timer() {
    assert!(StartKind::Spawned.needs_timer());
    assert!(StartKind::Adopted.needs_timer());
    assert!(StartKind::Scheduled.needs_timer());
    assert!(!StartKind::AlreadyRunning.needs_timer());
}

#[test]
fn a_batch_that_fully_started_refuses_nothing() {
    let requested = vec!["api".to_string(), "web".to_string()];
    let outcomes = vec![outcome_named("api"), outcome_named("web")];
    assert!(refused_services(&requested, &outcomes).is_empty());
}

#[test]
fn a_batch_that_half_started_refuses_the_rest() {
    let requested = vec!["api".to_string(), "web".to_string()];
    let outcomes = vec![outcome_named("api")];
    assert_eq!(refused_services(&requested, &outcomes), vec!["web"]);
}

fn outcome_named(name: &str) -> StartOutcome {
    StartOutcome {
        pm_id: 0,
        name: name.to_string(),
        pid: Some(100),
        kind: StartKind::Spawned,
    }
}

#[tokio::test]
async fn a_service_whose_dependency_is_probing_is_deferred() {
    let ports = FakePorts::new(0);
    let mut table = ProcessTable::new();
    let specs = [
        crate::ports_test_helpers::spec_probed("db"),
        spec_with_deps("web", &["db"]),
    ];
    let report = start_apps(&mut table, &specs, LOGS_DIR, &ports).await;
    assert_eq!(report.outcomes.len(), 2);
    let web = report
        .outcomes
        .iter()
        .find(|outcome| outcome.name == "web")
        .expect("web should be listed");
    assert_eq!(web.kind, StartKind::Deferred);
    assert_eq!(
        report.pending,
        vec![crate::start::DeferredStart {
            name: "web".to_string(),
            waiting_on: "db".to_string(),
        }]
    );
    assert!(report.failure.is_none());
}

#[tokio::test]
async fn a_service_whose_dependency_has_no_probe_starts_right_away() {
    let ports = FakePorts::new(0);
    let mut table = ProcessTable::new();
    let specs = [spec("db"), spec_with_deps("web", &["db"])];
    let report = start_apps(&mut table, &specs, LOGS_DIR, &ports).await;
    assert!(
        report
            .outcomes
            .iter()
            .all(|outcome| outcome.kind == StartKind::Spawned)
    );
    assert!(report.pending.is_empty());
}
