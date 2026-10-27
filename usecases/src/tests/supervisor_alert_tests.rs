use entities::{AppSpec, ReadyProbe};

use crate::{
    SupervisionEffect, SupervisionRequest,
    alert::{Alert, AlertKind},
    ports::ExitOutcome,
    ports_test_helpers::{
        FakePorts, LOGS_DIR, StaticResolver, spec, spec_probed, spec_with_deps, supervisor,
    },
    selector::AppSelector,
    start::start_apps,
    supervisor::Supervisor,
};

async fn launch(
    supervisor: &mut Supervisor,
    ports: &FakePorts,
    specs: &[AppSpec],
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

fn generation_of(effects: &[SupervisionEffect], name: &str) -> u64 {
    effects
        .iter()
        .find_map(|effect| {
            let SupervisionEffect::WatchExit {
                name: watched,
                generation,
                ..
            } = effect
            else {
                return None;
            };
            (watched == name).then_some(*generation)
        })
        .expect("the service should be watched")
}

fn alerts_in(effects: &[SupervisionEffect]) -> Vec<Alert> {
    effects
        .iter()
        .filter_map(|effect| {
            let SupervisionEffect::Notify(alert) = effect else {
                return None;
            };
            Some(alert.clone())
        })
        .collect()
}

fn alert(name: &str, kind: AlertKind) -> Vec<Alert> {
    vec![Alert {
        name: name.to_string(),
        kind,
    }]
}

fn one_shot(name: &str) -> AppSpec {
    AppSpec {
        autorestart: false,
        ..spec(name)
    }
}

#[tokio::test]
async fn a_failed_one_shot_raises_an_exit_alert() {
    let mut supervisor = supervisor();
    let ports = FakePorts::new(0);
    let started = launch(&mut supervisor, &ports, &[one_shot("job")]).await;
    let generation = generation_of(&started, "job");

    let effects = supervisor
        .on_exit("job", generation, ExitOutcome::Code(1), &ports)
        .await;

    assert_eq!(
        alerts_in(&effects),
        alert("job", AlertKind::Exited(ExitOutcome::Code(1)))
    );
}

#[tokio::test]
async fn a_clean_exit_raises_nothing() {
    let mut supervisor = supervisor();
    let ports = FakePorts::new(0);
    let started = launch(&mut supervisor, &ports, &[one_shot("job")]).await;
    let generation = generation_of(&started, "job");

    let effects = supervisor
        .on_exit("job", generation, ExitOutcome::Code(0), &ports)
        .await;

    assert_eq!(alerts_in(&effects), []);
}

#[tokio::test]
async fn an_operator_stop_raises_nothing() {
    let mut supervisor = supervisor();
    let ports = FakePorts::new(0);
    let started = launch(&mut supervisor, &ports, &[one_shot("job")]).await;
    let generation = generation_of(&started, "job");
    let request = SupervisionRequest::Stop(AppSelector::Name("job".to_string()));
    let (_outcome, stopping) = supervisor
        .handle(request, &StaticResolver::always(), &ports)
        .await;

    let effects = supervisor
        .on_exit("job", generation, ExitOutcome::Signalled, &ports)
        .await;

    assert_eq!(alerts_in(&stopping), []);
    assert_eq!(alerts_in(&effects), []);
}

#[tokio::test]
async fn a_looping_liveness_failure_alerts_once() {
    let mut supervisor = supervisor();
    let ports = FakePorts::new(0);
    let watched = AppSpec {
        liveness_probe: Some(ReadyProbe::Tcp {
            host: "127.0.0.1".to_string(),
            port: 8080,
        }),
        ..spec("api")
    };
    launch(&mut supervisor, &ports, &[watched]).await;
    ports.break_liveness();

    let first = supervisor.on_liveness_sample(1, 1, &ports).await;
    supervisor
        .table
        .find_by_name_mut("api")
        .expect("record present")
        .runtime
        .mark_online();
    let second = supervisor.on_liveness_sample(1, 1, &ports).await;

    assert_eq!(alerts_in(&first), alert("api", AlertKind::LivenessKilled));
    assert_eq!(alerts_in(&second), []);
}

#[tokio::test]
async fn a_ready_timeout_names_the_cancelled_downstream() {
    let mut supervisor = supervisor();
    let ports = FakePorts::new(0);
    let specs = [
        spec_probed("db"),
        spec_with_deps("web", &["db"]),
        spec_with_deps("api", &["web"]),
    ];
    let started = launch(&mut supervisor, &ports, &specs).await;
    let generation = generation_of(&started, "db");

    let effects = supervisor
        .on_ready_timeout("db", generation, "not ready", &ports)
        .await;
    let exited = supervisor
        .on_exit("db", generation, ExitOutcome::Signalled, &ports)
        .await;

    assert_eq!(
        alerts_in(&effects),
        alert(
            "db",
            AlertKind::ReadyFailed {
                downstream: vec!["web".to_string(), "api".to_string()],
            }
        )
    );
    assert_eq!(alerts_in(&exited), [], "one failure, one alert");
}

#[tokio::test]
async fn a_waiter_that_cannot_launch_raises_a_launch_alert() {
    let mut supervisor = supervisor();
    let ports = FakePorts::new(0);
    let specs = [spec_probed("db"), spec_with_deps("web", &["db"])];
    let started = launch(&mut supervisor, &ports, &specs).await;
    let generation = generation_of(&started, "db");
    ports.fail_spawn_for("web");

    let effects = supervisor.on_ready("db", generation, &ports).await;

    assert_eq!(alerts_in(&effects), alert("web", AlertKind::LaunchFailed));
}

#[tokio::test]
async fn an_automatic_restart_that_fails_raises_a_launch_alert() {
    let mut supervisor = supervisor();
    let ports = FakePorts::new(0);
    supervisor.table.upsert(spec("api"), 0);
    supervisor.queue_restart("api", 0);
    ports.fail_spawn_for("api");

    let effects = supervisor.on_restart("api", &ports).await;

    assert_eq!(alerts_in(&effects), alert("api", AlertKind::LaunchFailed));
}

#[tokio::test]
async fn stopping_a_probing_service_cancels_its_waiters_silently() {
    let mut supervisor = supervisor();
    let ports = FakePorts::new(0);
    let specs = [spec_probed("db"), spec_with_deps("web", &["db"])];
    launch(&mut supervisor, &ports, &specs).await;
    let request = SupervisionRequest::Stop(AppSelector::Name("db".to_string()));

    let (_outcome, effects) = supervisor
        .handle(request, &StaticResolver::always(), &ports)
        .await;

    assert_eq!(alerts_in(&effects), []);
}
