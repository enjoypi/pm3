use adapters::{SupervisionReply, SupervisionRequest};

use super::*;

async fn half_started_batch(harness: &mut Harness) -> SupervisionReply {
    let broken = unrunnable_script(harness);
    apps_file(harness, "web", SLEEPER);
    service_with_script(harness, "broken", &broken, &["web"]);
    harness
        .daemon
        .handle(SupervisionRequest::Start {
            services: vec!["web".to_string(), "broken".to_string()],
        })
        .await
        .expect("a half-started batch still answers with a summary")
}

#[tokio::test]
async fn a_half_started_batch_names_the_service_it_refused() {
    let mut harness = harness();
    let reply = half_started_batch(&mut harness).await;
    let SupervisionReply::Started {
        outcomes: _,
        refused,
        reason: _,
    } = reply
    else {
        panic!("start should answer with a start summary")
    };
    assert_eq!(refused, vec!["broken".to_string()]);
}

#[tokio::test]
async fn a_half_started_batch_keeps_the_service_it_did_start() {
    let mut harness = harness();
    let reply = half_started_batch(&mut harness).await;
    let SupervisionReply::Started {
        outcomes,
        refused: _,
        reason,
    } = reply
    else {
        panic!("start should answer with a start summary")
    };
    assert_eq!(outcomes.len(), 1);
    assert!(reason.is_some(), "the refusal must be explained");
}

#[tokio::test]
async fn a_batch_that_started_nothing_is_refused_outright() {
    let mut harness = harness();
    let broken = unrunnable_script(&harness);
    service_with_script(&harness, "broken", &broken, &[]);
    let refused = harness
        .daemon
        .handle(SupervisionRequest::Start {
            services: vec!["broken".to_string()],
        })
        .await;
    assert!(refused.is_err(), "nothing started, so nothing is kept");
}

#[tokio::test]
async fn stopping_everything_force_kills_whatever_outlived_the_grace_period() {
    let mut harness = harness_with_kill_timeout(0);
    let started = start_one(&mut harness, "web", SLEEPER).await;
    let pid = started.pid.expect("a pid");
    harness
        .daemon
        .handle(SupervisionRequest::StopAll)
        .await
        .expect("should stop everything");

    let (name, generation, killed, token) = next_force_kill(&mut harness.events).await;
    assert_eq!(
        (name.as_str(), killed, token.is_some()),
        ("web", pid, true),
        "the sweep must target the stopped service with its identity token"
    );
    harness
        .daemon
        .on_force_kill(&name, generation, killed, token.as_deref())
        .await;
    for _attempt in 0..100 {
        if harness.ports.tracked_pids().await.is_empty() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("the force kill should reap the lingering child");
}

#[tokio::test]
async fn stopping_everything_sweeps_a_tracked_pid_that_outlives_the_grace_period() {
    let mut harness = harness_with_kill_timeout(120);
    adapters::ProcessLauncher::adopt(&*harness.ports, u32::MAX).await;

    harness
        .daemon
        .handle(SupervisionRequest::StopAll)
        .await
        .expect("should stop everything");

    let (name, generation, pid, token) = next_force_kill(&mut harness.events).await;
    assert_eq!(pid, u32::MAX, "the sweep must target the stray pid");
    harness
        .daemon
        .on_force_kill(&name, generation, pid, token.as_deref())
        .await;
    assert_eq!(
        harness.ports.tracked_pids().await,
        vec![u32::MAX],
        "an unsignalable pid stays tracked after the sweep"
    );
}

#[tokio::test]
async fn shutting_down_force_kills_only_the_services_that_were_stopping() {
    let mut harness = harness_with_kill_timeout(0);
    let kept = start_one(&mut harness, "web", SLEEPER).await;
    start_one(&mut harness, "db", SLEEPER).await;
    harness
        .daemon
        .handle(SupervisionRequest::Stop(selector("db")))
        .await
        .expect("should stop db");

    harness.daemon.shutdown().await;

    loop {
        let event = next_event(&mut harness.events).await;
        if matches!(event, DaemonEvent::Exited { .. }) {
            break;
        }
    }
    assert_eq!(
        harness.ports.tracked_pids().await,
        vec![kept.pid.expect("a pid")],
        "a service that was not stopping must survive the shutdown sweep"
    );
}
