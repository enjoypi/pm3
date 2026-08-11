use super::*;

#[tokio::test]
async fn a_timeout_cascades_to_the_waiting_dependencies() {
    let mut supervisor = supervisor();
    let ports = FakePorts::new(0);
    let specs = vec![
        spec_probed("db"),
        spec_with_deps("web", &["db"]),
        spec_with_deps("api", &["web"]),
    ];
    let generation = start_batch(&mut supervisor, &ports, &specs).await;

    supervisor
        .on_ready_timeout("db", generation, "not ready", &ports)
        .await;

    assert_eq!(status_of(&supervisor, "web"), ProcessStatus::Errored);
    assert_eq!(status_of(&supervisor, "api"), ProcessStatus::Errored);
}

#[tokio::test]
async fn a_timeout_with_a_waiter_that_already_started_keeps_it_running() {
    let mut supervisor = supervisor();
    let ports = FakePorts::new(0);
    let effect = start_probed(&mut supervisor, &ports, "db").await;
    let generation = generation_of(&effect);
    let specs = vec![spec("web")];
    start_apps(&mut supervisor.table, &specs, LOGS_DIR, &ports).await;
    supervisor
        .waiters
        .insert("db".to_string(), vec!["web".to_string()]);

    supervisor
        .on_ready_timeout("db", generation, "not ready", &ports)
        .await;

    assert_eq!(status_of(&supervisor, "web"), ProcessStatus::Online);
}

#[tokio::test]
async fn a_timeout_still_cascades_when_the_terminate_fails() {
    let mut supervisor = supervisor();
    let ports = FakePorts::new(0);
    let specs = vec![spec_probed("db"), spec_with_deps("web", &["db"])];
    let generation = start_batch(&mut supervisor, &ports, &specs).await;
    let pid = supervisor
        .table
        .find_by_name("db")
        .and_then(|record| record.runtime.pid)
        .expect("db should be running");
    ports.fail_signal_for(pid);

    supervisor
        .on_ready_timeout("db", generation, "not ready", &ports)
        .await;

    assert_eq!(status_of(&supervisor, "web"), ProcessStatus::Errored);
}

#[tokio::test]
async fn a_waiter_that_fails_to_launch_is_marked_errored_and_cascades() {
    let mut supervisor = supervisor();
    let ports = FakePorts::new(0);
    let specs = vec![
        spec_probed("db"),
        spec_with_deps("web", &["db"]),
        spec_with_deps("api", &["web"]),
    ];
    let generation = start_batch(&mut supervisor, &ports, &specs).await;
    ports.fail_spawn_for("web");

    supervisor.on_ready("db", generation, &ports).await;

    assert_eq!(status_of(&supervisor, "web"), ProcessStatus::Errored);
    assert_eq!(status_of(&supervisor, "api"), ProcessStatus::Errored);
}

#[tokio::test]
async fn stopping_a_probe_in_flight_cancels_it_and_drops_its_waiters() {
    let mut supervisor = supervisor();
    let ports = FakePorts::new(0);
    let specs = vec![
        spec_probed("db"),
        spec_with_deps("web", &["db"]),
        spec_with_deps("api", &["db"]),
    ];
    start_batch(&mut supervisor, &ports, &specs).await;

    let mut effects = Vec::new();
    supervisor.cancel_ready("web", &mut effects);

    assert!(effects.iter().any(
        |candidate| matches!(candidate, SupervisionEffect::CancelReady { name } if name == "web")
    ));
    let remaining = supervisor
        .waiters
        .get("db")
        .expect("api should still be waiting");
    assert_eq!(remaining, &vec!["api".to_string()]);
}

#[tokio::test]
async fn stopping_a_dependency_fails_its_waiters() {
    let mut supervisor = supervisor();
    let ports = FakePorts::new(0);
    let specs = vec![spec_probed("db"), spec_with_deps("web", &["db"])];
    start_batch(&mut supervisor, &ports, &specs).await;

    let mut effects = Vec::new();
    supervisor.cancel_ready("db", &mut effects);

    assert_eq!(status_of(&supervisor, "web"), ProcessStatus::Errored);
}

#[tokio::test]
async fn an_adopted_service_with_a_probe_does_not_await_readiness() {
    let mut supervisor = supervisor();
    let ports = FakePorts::new(0);
    let effect = start_probed(&mut supervisor, &ports, "web").await;
    let generation = generation_of(&effect);
    supervisor.on_ready("web", generation, &ports).await;

    let outcome = crate::start::StartOutcome {
        pm_id: 0,
        name: "web".to_string(),
        pid: Some(42),
        kind: StartKind::Adopted,
    };
    let mut effects = Vec::new();
    supervisor.watch(&outcome, &mut effects);

    assert!(
        !effects
            .iter()
            .any(|candidate| matches!(candidate, SupervisionEffect::AwaitReady { .. })),
        "an adopted process is already serving"
    );
}

#[tokio::test]
async fn a_ready_transition_reports_a_save_failure() {
    let mut supervisor = supervisor();
    let ports = FakePorts::new(0);
    let effect = start_probed(&mut supervisor, &ports, "web").await;
    let generation = generation_of(&effect);
    ports.fail_save();

    supervisor.on_ready("web", generation, &ports).await;

    assert_eq!(status_of(&supervisor, "web"), ProcessStatus::Online);
}

#[tokio::test]
async fn a_timeout_reports_a_save_failure() {
    let mut supervisor = supervisor();
    let ports = FakePorts::new(0);
    let effect = start_probed(&mut supervisor, &ports, "web").await;
    let generation = generation_of(&effect);
    ports.fail_save();

    supervisor
        .on_ready_timeout("web", generation, "not ready", &ports)
        .await;

    assert_eq!(status_of(&supervisor, "web"), ProcessStatus::Stopping);
}

#[tokio::test]
async fn a_failed_probe_settlement_reports_a_save_failure() {
    let mut supervisor = supervisor();
    let ports = FakePorts::new(0);
    let effect = start_probed(&mut supervisor, &ports, "web").await;
    let generation = generation_of(&effect);
    supervisor
        .on_ready_timeout("web", generation, "not ready", &ports)
        .await;
    ports.fail_save();

    supervisor
        .on_exit(
            "web",
            generation,
            crate::ports::ExitOutcome::Code(143),
            &ports,
        )
        .await;

    assert_eq!(status_of(&supervisor, "web"), ProcessStatus::Errored);
}

#[tokio::test]
async fn a_released_waiter_tolerates_a_save_failure() {
    let mut supervisor = supervisor();
    let ports = FakePorts::new(0);
    let specs = vec![spec_probed("db"), spec_with_deps("web", &["db"])];
    let generation = start_batch(&mut supervisor, &ports, &specs).await;
    ports.fail_save();

    supervisor.on_ready("db", generation, &ports).await;

    assert_eq!(status_of(&supervisor, "web"), ProcessStatus::Online);
}

#[tokio::test]
async fn a_waiter_with_two_dependencies_launches_only_after_both_are_ready() {
    let mut supervisor = supervisor();
    let ports = FakePorts::new(0);
    let specs = vec![
        spec_probed("db"),
        spec_probed("cache"),
        spec_with_deps("web", &["db", "cache"]),
    ];
    let effects = start_batch_effects(&mut supervisor, &ports, &specs).await;

    supervisor
        .on_ready("db", generation_for(&effects, "db"), &ports)
        .await;
    assert_eq!(status_of(&supervisor, "web"), ProcessStatus::Stopped);

    supervisor
        .on_ready("cache", generation_for(&effects, "cache"), &ports)
        .await;
    assert_eq!(status_of(&supervisor, "web"), ProcessStatus::Online);
}

#[tokio::test]
async fn a_failed_dependency_blocks_the_waiter_even_after_the_other_readies() {
    let mut supervisor = supervisor();
    let ports = FakePorts::new(0);
    let specs = vec![
        spec_probed("db"),
        spec_probed("cache"),
        spec_with_deps("web", &["db", "cache"]),
    ];
    let effects = start_batch_effects(&mut supervisor, &ports, &specs).await;

    supervisor
        .on_ready_timeout("db", generation_for(&effects, "db"), "not ready", &ports)
        .await;
    assert_eq!(status_of(&supervisor, "web"), ProcessStatus::Errored);

    supervisor
        .on_ready("cache", generation_for(&effects, "cache"), &ports)
        .await;
    assert_eq!(status_of(&supervisor, "web"), ProcessStatus::Errored);
    assert!(!ports.spawned_names().contains(&"web".to_string()));
}

#[tokio::test]
async fn a_released_waiter_is_persisted_before_the_daemon_can_lose_it() {
    let mut supervisor = supervisor();
    let ports = FakePorts::new(0);
    let specs = vec![spec_probed("db"), spec_with_deps("web", &["db"])];
    let generation = start_batch(&mut supervisor, &ports, &specs).await;

    supervisor.on_ready("db", generation, &ports).await;

    let stored = ports.stored();
    let web = stored
        .iter()
        .find(|record| record.runtime.name == "web")
        .expect("web should be persisted once it launched");
    assert_eq!(web.runtime.status, ProcessStatus::Online);
    assert!(web.runtime.pid.is_some());
}

#[tokio::test]
async fn a_waiter_launch_failure_reports_a_save_failure() {
    let mut supervisor = supervisor();
    let ports = FakePorts::new(0);
    let specs = vec![spec_probed("db"), spec_with_deps("web", &["db"])];
    let generation = start_batch(&mut supervisor, &ports, &specs).await;
    ports.fail_spawn_for("web");
    ports.fail_save();

    supervisor.on_ready("db", generation, &ports).await;

    assert_eq!(status_of(&supervisor, "web"), ProcessStatus::Errored);
}

#[tokio::test]
async fn a_waiter_with_its_own_probe_keeps_the_chain_waiting() {
    let mut supervisor = supervisor();
    let ports = FakePorts::new(0);
    let specs = vec![
        spec_probed("db"),
        entities::AppSpec {
            ready_probe: Some(entities::ReadyProbe::Exec {
                command: vec!["/usr/bin/true".to_string()],
            }),
            ..spec_with_deps("web", &["db"])
        },
        spec_with_deps("api", &["web"]),
    ];
    let generation = start_batch(&mut supervisor, &ports, &specs).await;

    supervisor.on_ready("db", generation, &ports).await;

    assert_eq!(status_of(&supervisor, "web"), ProcessStatus::Launching);
    assert_eq!(status_of(&supervisor, "api"), ProcessStatus::Stopped);
}
