use entities::AppSpec;

use super::*;
use crate::{
    ports::SpecResolveError,
    ports_test_helpers::{FakePorts, LOGS_DIR, spec},
    start::start_apps,
};

const KILL_TIMEOUT_MS: u64 = 1600;
const READY_TIMEOUT_MS: u64 = 30000;
const READY_POLL_MS: u64 = 200;

struct NoResolver;

impl SpecResolver for NoResolver {
    async fn prepare(&self, name: &str) -> Result<AppSpec, SpecResolveError> {
        Err(SpecResolveError::Missing {
            name: name.to_string(),
            reason: "this resolver resolves nothing".to_string(),
        })
    }
}

fn supervisor() -> Supervisor {
    Supervisor::new(
        LOGS_DIR.to_string(),
        KILL_TIMEOUT_MS,
        READY_TIMEOUT_MS,
        READY_POLL_MS,
    )
}

async fn running_supervisor(ports: &FakePorts) -> Supervisor {
    let mut supervisor = supervisor();
    start_apps(&mut supervisor.table, &[spec("api")], LOGS_DIR, ports).await;
    supervisor
}

fn arms_force_kill(effects: &[SupervisionEffect], pid: u32) -> bool {
    effects.iter().any(|candidate| {
        matches!(
            candidate,
            SupervisionEffect::ScheduleForceKill { pid: covered, .. } if *covered == pid
        )
    })
}

#[tokio::test]
async fn a_save_failure_during_stop_all_still_arms_every_force_kill() {
    let ports = FakePorts::new(1000);
    let mut supervisor = running_supervisor(&ports).await;
    ports.fail_save();
    let (outcome, effects) = supervisor
        .handle(SupervisionRequest::StopAll, &NoResolver, &ports)
        .await;
    assert!(outcome.is_ok(), "got: {outcome:?}");
    assert!(arms_force_kill(&effects, 100), "got: {effects:?}");
}

#[tokio::test]
async fn a_save_failure_after_the_stop_signal_still_arms_the_force_kill() {
    let ports = FakePorts::new(1000);
    let mut supervisor = running_supervisor(&ports).await;
    ports.fail_save();
    let (outcome, effects) = supervisor
        .handle(
            SupervisionRequest::Stop(AppSelector::Id(0)),
            &NoResolver,
            &ports,
        )
        .await;
    assert!(outcome.is_err());
    assert!(arms_force_kill(&effects, 100), "got: {effects:?}");
}

#[tokio::test]
async fn a_save_failure_after_the_delete_signal_still_arms_the_force_kill() {
    let ports = FakePorts::new(1000);
    let mut supervisor = running_supervisor(&ports).await;
    ports.fail_save();
    let (outcome, effects) = supervisor
        .handle(
            SupervisionRequest::Delete(AppSelector::Id(0)),
            &NoResolver,
            &ports,
        )
        .await;
    assert!(outcome.is_err());
    assert!(arms_force_kill(&effects, 100), "got: {effects:?}");
}

#[tokio::test]
async fn a_failed_delete_keeps_the_draining_record_tracked() {
    let ports = FakePorts::new(1000);
    let mut supervisor = running_supervisor(&ports).await;
    ports.fail_save();
    let (outcome, _effects) = supervisor
        .handle(
            SupervisionRequest::Delete(AppSelector::Id(0)),
            &NoResolver,
            &ports,
        )
        .await;
    assert!(outcome.is_err());
    let record = supervisor
        .table
        .find(&AppSelector::Id(0))
        .expect("a service whose deletion was not persisted must stay tracked");
    assert_eq!(record.runtime.status, ProcessStatus::Stopping);
}

#[tokio::test]
async fn stopping_an_unknown_service_reports_not_found_without_effects() {
    let ports = FakePorts::new(1000);
    let mut supervisor = supervisor();
    let (outcome, effects) = supervisor
        .handle(
            SupervisionRequest::Stop(AppSelector::Id(9)),
            &NoResolver,
            &ports,
        )
        .await;
    assert!(outcome.is_err());
    assert!(effects.is_empty(), "got: {effects:?}");
}

fn force_kill_effect(effects: &[SupervisionEffect]) -> (u64, u32, Option<String>) {
    let effect = effects
        .iter()
        .find(|candidate| matches!(candidate, SupervisionEffect::ScheduleForceKill { .. }))
        .expect("a force kill should be scheduled");
    let SupervisionEffect::ScheduleForceKill {
        generation,
        pid,
        token,
        ..
    } = effect
    else {
        panic!("a force kill should be scheduled")
    };
    (*generation, *pid, token.clone())
}

#[tokio::test]
async fn a_force_kill_for_a_replaced_instance_still_fires_when_the_token_matches() {
    let ports = FakePorts::new(1000);
    let mut supervisor = running_supervisor(&ports).await;
    ports.make_stubborn(100);
    let (outcome, effects) = supervisor
        .handle(
            SupervisionRequest::Delete(AppSelector::Id(0)),
            &NoResolver,
            &ports,
        )
        .await;
    assert!(outcome.is_ok());
    let (generation, pid, token) = force_kill_effect(&effects);
    supervisor.timers.bump("api");

    supervisor
        .on_force_kill("api", generation, pid, token.as_deref(), &ports)
        .await;

    assert_eq!(ports.force_killed(), vec![pid]);
}

#[tokio::test]
async fn a_force_kill_for_a_replaced_instance_without_a_token_stays_dropped() {
    let ports = FakePorts::new(1000);
    let mut supervisor = running_supervisor(&ports).await;
    let (outcome, effects) = supervisor
        .handle(
            SupervisionRequest::Delete(AppSelector::Id(0)),
            &NoResolver,
            &ports,
        )
        .await;
    assert!(outcome.is_ok());
    let (generation, pid, _token) = force_kill_effect(&effects);
    supervisor.timers.bump("api");

    supervisor
        .on_force_kill("api", generation, pid, None, &ports)
        .await;

    assert!(ports.force_killed().is_empty());
}

#[tokio::test]
async fn a_refused_stop_signal_arms_no_force_kill() {
    let ports = FakePorts::new(1000);
    let mut supervisor = running_supervisor(&ports).await;
    ports.fail_signal_for(100);
    let (outcome, effects) = supervisor
        .handle(
            SupervisionRequest::Stop(AppSelector::Id(0)),
            &NoResolver,
            &ports,
        )
        .await;
    assert!(outcome.is_err());
    assert!(!arms_force_kill(&effects, 100), "got: {effects:?}");
    let record = supervisor
        .table
        .find(&AppSelector::Id(0))
        .expect("record present");
    assert_eq!(record.runtime.status, ProcessStatus::Online);
}
