use super::*;

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
            waiting_on: vec!["db".to_string()],
        }]
    );
    assert!(report.failure.is_none());
}

#[tokio::test]
async fn a_service_with_two_probing_dependencies_waits_for_both() {
    let ports = FakePorts::new(0);
    let mut table = ProcessTable::new();
    let specs = [
        crate::ports_test_helpers::spec_probed("db"),
        crate::ports_test_helpers::spec_probed("cache"),
        spec_with_deps("web", &["db", "cache"]),
    ];
    let report = start_apps(&mut table, &specs, LOGS_DIR, &ports).await;
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
            waiting_on: vec!["db".to_string(), "cache".to_string()],
        }]
    );
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

#[test]
fn a_reply_with_nothing_refused_or_unsaved_settles_as_committed() {
    assert_eq!(settle_start(Vec::new(), None), StartSettlement::Committed);
}

#[test]
fn refused_services_settle_as_partial_even_when_the_reply_is_unsaved() {
    assert_eq!(
        settle_start(vec!["web".to_string()], Some("dump write failed")),
        StartSettlement::Partial {
            refused: vec!["web".to_string()]
        }
    );
}

#[test]
fn an_unsaved_reply_with_no_refusal_settles_as_unsaved() {
    assert_eq!(
        settle_start(Vec::new(), Some("dump write failed")),
        StartSettlement::Unsaved
    );
}
