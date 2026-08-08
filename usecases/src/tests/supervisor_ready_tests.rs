use entities::ProcessStatus;

use super::*;
use crate::{
    SupervisionEffect,
    ports_test_helpers::{FakePorts, LOGS_DIR, spec, spec_probed, spec_with_deps},
    start::{StartKind, start_apps},
};

const KILL_TIMEOUT_MS: u64 = 1600;
const READY_TIMEOUT_MS: u64 = 30000;
const READY_POLL_MS: u64 = 200;

fn supervisor() -> Supervisor {
    Supervisor::new(
        LOGS_DIR.to_string(),
        KILL_TIMEOUT_MS,
        READY_TIMEOUT_MS,
        READY_POLL_MS,
    )
}

async fn start_probed(
    supervisor: &mut Supervisor,
    ports: &FakePorts,
    name: &str,
) -> SupervisionEffect {
    let specs = vec![spec_probed(name)];
    let report = start_apps(&mut supervisor.table, &specs, LOGS_DIR, ports).await;
    let mut effects = Vec::new();
    supervisor.watch_all(&report.outcomes, &mut effects);
    effects
        .into_iter()
        .find(|candidate| matches!(candidate, SupervisionEffect::AwaitReady { .. }))
        .expect("a probed service should arm a readiness watch")
}

fn generation_of(effect: &SupervisionEffect) -> u64 {
    let SupervisionEffect::AwaitReady { generation, .. } = effect else {
        panic!("expected an await-ready effect")
    };
    *generation
}

fn status_of(supervisor: &Supervisor, name: &str) -> ProcessStatus {
    supervisor
        .table
        .find_by_name(name)
        .expect("the record should exist")
        .runtime
        .status
}

#[tokio::test]
async fn a_probe_passing_marks_the_service_online() {
    let mut supervisor = supervisor();
    let ports = FakePorts::new(0);
    let effect = start_probed(&mut supervisor, &ports, "web").await;
    assert_eq!(status_of(&supervisor, "web"), ProcessStatus::Launching);

    let effects = supervisor
        .on_ready("web", generation_of(&effect), &ports)
        .await;

    assert_eq!(status_of(&supervisor, "web"), ProcessStatus::Online);
    assert!(effects.is_empty());
}

#[tokio::test]
async fn a_ready_event_with_a_stale_generation_is_dropped() {
    let mut supervisor = supervisor();
    let ports = FakePorts::new(0);
    start_probed(&mut supervisor, &ports, "web").await;

    let effects = supervisor.on_ready("web", 999, &ports).await;

    assert_eq!(status_of(&supervisor, "web"), ProcessStatus::Launching);
    assert!(effects.is_empty());
}

#[tokio::test]
async fn a_ready_event_for_a_service_no_longer_launching_is_dropped() {
    let mut supervisor = supervisor();
    let ports = FakePorts::new(0);
    let effect = start_probed(&mut supervisor, &ports, "web").await;
    let generation = generation_of(&effect);
    supervisor.on_ready("web", generation, &ports).await;

    let effects = supervisor.on_ready("web", generation, &ports).await;

    assert_eq!(status_of(&supervisor, "web"), ProcessStatus::Online);
    assert!(effects.is_empty());
}

#[tokio::test]
async fn a_ready_service_releases_its_waiting_dependencies() {
    let mut supervisor = supervisor();
    let ports = FakePorts::new(0);
    let specs = vec![spec_probed("db"), spec_with_deps("web", &["db"])];
    let generation = start_batch(&mut supervisor, &ports, &specs).await;
    assert_eq!(status_of(&supervisor, "web"), ProcessStatus::Stopped);

    supervisor.on_ready("db", generation, &ports).await;

    assert_eq!(status_of(&supervisor, "web"), ProcessStatus::Online);
}

#[tokio::test]
async fn a_readied_chain_launches_each_waiter_in_turn() {
    let mut supervisor = supervisor();
    let ports = FakePorts::new(0);
    let specs = vec![
        spec_probed("db"),
        spec_with_deps("web", &["db"]),
        spec_with_deps("api", &["web"]),
    ];
    let generation = start_batch(&mut supervisor, &ports, &specs).await;

    supervisor.on_ready("db", generation, &ports).await;

    assert_eq!(status_of(&supervisor, "web"), ProcessStatus::Online);
    assert_eq!(status_of(&supervisor, "api"), ProcessStatus::Online);
}

#[tokio::test]
async fn a_timeout_stops_the_service_and_marks_it_errored_on_exit() {
    let mut supervisor = supervisor();
    let ports = FakePorts::new(0);
    let effect = start_probed(&mut supervisor, &ports, "web").await;
    let generation = generation_of(&effect);
    let pid = supervisor
        .table
        .find_by_name("web")
        .and_then(|record| record.runtime.pid)
        .expect("web should be running");

    let effects = supervisor
        .on_ready_timeout("web", generation, "not ready", &ports)
        .await;

    assert_eq!(status_of(&supervisor, "web"), ProcessStatus::Stopping);
    assert!(ports.terminated().contains(&pid));
    assert!(
        effects
            .iter()
            .any(|candidate| matches!(candidate, SupervisionEffect::ScheduleForceKill { .. }))
    );
    let after_exit = supervisor
        .on_exit(
            "web",
            generation,
            crate::ports::ExitOutcome::Code(143),
            &ports,
        )
        .await;
    assert_eq!(status_of(&supervisor, "web"), ProcessStatus::Errored);
    assert!(
        !after_exit
            .iter()
            .any(|candidate| matches!(candidate, SupervisionEffect::ScheduleRestart { .. })),
        "a probe failure must not restart the service"
    );
}

#[tokio::test]
async fn a_timeout_with_a_stale_generation_is_dropped() {
    let mut supervisor = supervisor();
    let ports = FakePorts::new(0);
    start_probed(&mut supervisor, &ports, "web").await;

    let effects = supervisor
        .on_ready_timeout("web", 999, "not ready", &ports)
        .await;

    assert_eq!(status_of(&supervisor, "web"), ProcessStatus::Launching);
    assert!(effects.is_empty());
}

#[tokio::test]
async fn a_timeout_for_a_service_no_longer_launching_is_dropped() {
    let mut supervisor = supervisor();
    let ports = FakePorts::new(0);
    let effect = start_probed(&mut supervisor, &ports, "web").await;
    let generation = generation_of(&effect);
    supervisor.on_ready("web", generation, &ports).await;

    let effects = supervisor
        .on_ready_timeout("web", generation, "not ready", &ports)
        .await;

    assert_eq!(status_of(&supervisor, "web"), ProcessStatus::Online);
    assert!(effects.is_empty());
}

async fn start_batch(
    supervisor: &mut Supervisor,
    ports: &FakePorts,
    specs: &[entities::AppSpec],
) -> u64 {
    let report = start_apps(&mut supervisor.table, specs, LOGS_DIR, ports).await;
    let mut effects = Vec::new();
    supervisor.watch_all(&report.outcomes, &mut effects);
    for deferred in report.pending {
        supervisor
            .waiters
            .entry(deferred.waiting_on)
            .or_default()
            .push(deferred.name);
    }
    let armed = effects
        .iter()
        .find(|candidate| matches!(candidate, SupervisionEffect::AwaitReady { .. }))
        .expect("the batch should arm a readiness watch");
    generation_of(armed)
}

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
