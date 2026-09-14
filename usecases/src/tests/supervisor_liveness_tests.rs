use entities::{ProcessStatus, ReadyProbe};

use super::*;
use crate::{
    SupervisionEffect,
    ports_test_helpers::{FakePorts, LOGS_DIR, spec},
    start::start_apps,
};

const KILL_TIMEOUT_MS: u64 = 1600;
const READY_TIMEOUT_MS: u64 = 30000;
const READY_POLL_MS: u64 = 200;
const INTERVAL_MS: u64 = 30000;
const THRESHOLD: u32 = 2;

fn supervisor() -> Supervisor {
    Supervisor::new(
        LOGS_DIR.to_string(),
        KILL_TIMEOUT_MS,
        READY_TIMEOUT_MS,
        READY_POLL_MS,
    )
}

async fn start_watched(supervisor: &mut Supervisor, ports: &FakePorts, name: &str) {
    let mut candidate = spec(name);
    candidate.liveness_probe = Some(ReadyProbe::Tcp {
        host: "127.0.0.1".to_string(),
        port: 8080,
    });
    start_apps(&mut supervisor.table, &[candidate], LOGS_DIR, ports).await;
    let record = supervisor
        .table
        .find_by_name_mut(name)
        .expect("record present");
    record.runtime.mark_online();
}

fn status_of(supervisor: &Supervisor, name: &str) -> ProcessStatus {
    supervisor
        .table
        .find_by_name(name)
        .expect("record present")
        .runtime
        .status
}

fn failures_of(supervisor: &Supervisor, name: &str) -> u32 {
    supervisor
        .table
        .find_by_name(name)
        .expect("record present")
        .runtime
        .liveness_failures
}

#[tokio::test]
async fn a_sample_arms_the_following_one() {
    let ports = FakePorts::new(1000);
    let mut supervisor = supervisor();

    let effects = supervisor
        .on_liveness_sample(INTERVAL_MS, THRESHOLD, &ports)
        .await;

    assert_eq!(
        effects,
        vec![SupervisionEffect::ScheduleLivenessSample {
            delay_ms: INTERVAL_MS,
        }],
        "an empty table must still keep the beat"
    );
}

#[tokio::test]
async fn a_passing_probe_leaves_the_service_alone() {
    let ports = FakePorts::new(1000);
    let mut supervisor = supervisor();
    start_watched(&mut supervisor, &ports, "api").await;

    supervisor
        .on_liveness_sample(INTERVAL_MS, THRESHOLD, &ports)
        .await;

    assert_eq!(status_of(&supervisor, "api"), ProcessStatus::Online);
    assert_eq!(failures_of(&supervisor, "api"), 0);
}

#[tokio::test]
async fn a_single_failure_only_counts() {
    let ports = FakePorts::new(1000);
    let mut supervisor = supervisor();
    start_watched(&mut supervisor, &ports, "api").await;
    ports.break_liveness();

    supervisor
        .on_liveness_sample(INTERVAL_MS, THRESHOLD, &ports)
        .await;

    assert_eq!(status_of(&supervisor, "api"), ProcessStatus::Online);
    assert_eq!(failures_of(&supervisor, "api"), 1);
}

#[tokio::test]
async fn the_threshold_takes_the_service_down() {
    let ports = FakePorts::new(1000);
    let mut supervisor = supervisor();
    start_watched(&mut supervisor, &ports, "api").await;
    ports.break_liveness();

    supervisor
        .on_liveness_sample(INTERVAL_MS, THRESHOLD, &ports)
        .await;
    supervisor
        .on_liveness_sample(INTERVAL_MS, THRESHOLD, &ports)
        .await;

    assert_eq!(status_of(&supervisor, "api"), ProcessStatus::Stopping);
    assert_eq!(ports.terminated(), vec![100]);
}

#[tokio::test]
async fn a_liveness_restart_answers_to_the_breaker() {
    let ports = FakePorts::new(1000);
    let mut supervisor = supervisor();
    start_watched(&mut supervisor, &ports, "api").await;
    ports.break_liveness();

    supervisor.on_liveness_sample(INTERVAL_MS, 1, &ports).await;

    let record = supervisor
        .table
        .find_by_name("api")
        .expect("record present");
    assert!(
        record.runtime.supervised_restart,
        "the breaker must judge a probe-driven restart"
    );
}

#[tokio::test]
async fn a_recovering_probe_clears_the_tally() {
    let ports = FakePorts::new(1000);
    let mut supervisor = supervisor();
    start_watched(&mut supervisor, &ports, "api").await;
    ports.break_liveness();
    supervisor
        .on_liveness_sample(INTERVAL_MS, THRESHOLD, &ports)
        .await;
    assert_eq!(failures_of(&supervisor, "api"), 1);

    let healthy = FakePorts::new(1000);
    supervisor
        .on_liveness_sample(INTERVAL_MS, THRESHOLD, &healthy)
        .await;

    assert_eq!(failures_of(&supervisor, "api"), 0);
    assert_eq!(status_of(&supervisor, "api"), ProcessStatus::Online);
}

#[tokio::test]
async fn a_service_without_a_probe_is_never_sampled() {
    let ports = FakePorts::new(1000);
    let mut supervisor = supervisor();
    start_apps(&mut supervisor.table, &[spec("plain")], LOGS_DIR, &ports).await;
    let record = supervisor
        .table
        .find_by_name_mut("plain")
        .expect("record present");
    record.runtime.mark_online();
    ports.break_liveness();

    supervisor
        .on_liveness_sample(INTERVAL_MS, THRESHOLD, &ports)
        .await;

    assert_eq!(status_of(&supervisor, "plain"), ProcessStatus::Online);
}
