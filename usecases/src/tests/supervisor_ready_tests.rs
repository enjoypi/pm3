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
async fn a_reclaimed_survivor_still_probing_awaits_readiness_again() {
    let mut supervisor = supervisor();
    let ports = FakePorts::new(0);
    let spec = spec_probed("web");
    let mut runtime = entities::ProcessRuntime::new(0, "web".to_string(), 1000);
    runtime.mark_launched(7, 1000);
    runtime.status = ProcessStatus::Launching;
    runtime.identity = Some(entities::ProcessIdentity {
        token: crate::ports_test_helpers::live_token(7),
        launch_digest: crate::ports::Fingerprinter::digest(
            &ports,
            &crate::fingerprint::render_identity(&spec),
        ),
        binary_digest: format!(
            "{}{}",
            crate::ports_test_helpers::FILE_DIGEST_PREFIX,
            spec.script
        ),
    });
    let record = crate::record::ProcessRecord { spec, runtime };
    ports.seed_live(7, &crate::ports_test_helpers::live_token(7));
    ports.seed_stored(vec![record]);

    let effects = supervisor.resurrect_saved(&ports).await;

    assert!(
        effects.iter().any(
            |candidate| matches!(candidate, SupervisionEffect::AwaitReady { name, .. } if name == "web")
        ),
        "got: {effects:?}"
    );
    assert_eq!(status_of(&supervisor, "web"), ProcessStatus::Launching);
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
    let effects = start_batch_effects(supervisor, ports, specs).await;
    let armed = effects
        .iter()
        .find(|candidate| matches!(candidate, SupervisionEffect::AwaitReady { .. }))
        .expect("the batch should arm a readiness watch");
    generation_of(armed)
}

async fn start_batch_effects(
    supervisor: &mut Supervisor,
    ports: &FakePorts,
    specs: &[entities::AppSpec],
) -> Vec<SupervisionEffect> {
    let report = start_apps(&mut supervisor.table, specs, LOGS_DIR, ports).await;
    let mut effects = Vec::new();
    supervisor.watch_all(&report.outcomes, &mut effects);
    for deferred in report.pending {
        for dependency in &deferred.waiting_on {
            supervisor
                .waiters
                .entry(dependency.clone())
                .or_default()
                .push(deferred.name.clone());
        }
    }
    effects
}

fn generation_for(effects: &[SupervisionEffect], name: &str) -> u64 {
    let effect = effects
        .iter()
        .find(|candidate| {
            matches!(
                candidate,
                SupervisionEffect::AwaitReady { name: service, .. } if service == name
            )
        })
        .expect("the service should arm a readiness watch");
    let SupervisionEffect::AwaitReady { generation, .. } = effect else {
        panic!("the service should arm a readiness watch")
    };
    *generation
}

#[path = "supervisor_ready_cascade_tests.rs"]
mod cascade;
