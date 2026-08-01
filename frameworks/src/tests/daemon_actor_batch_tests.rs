use super::*;

async fn half_started_batch(harness: &mut Harness) -> DaemonReply {
    let broken = unrunnable_script(harness);
    apps_file(harness, "web", SLEEPER);
    service_with_script(harness, "broken", &broken, &["web"]);
    harness
        .daemon
        .handle(DaemonRequest::Start {
            services: vec!["web".to_string(), "broken".to_string()],
        })
        .await
        .expect("a half-started batch still answers with a summary")
}

#[tokio::test]
async fn a_half_started_batch_names_the_service_it_refused() {
    let mut harness = harness();
    let reply = half_started_batch(&mut harness).await;
    let DaemonReply::Started {
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
    let DaemonReply::Started {
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
        .handle(DaemonRequest::Start {
            services: vec!["broken".to_string()],
        })
        .await;
    assert!(refused.is_err(), "nothing started, so nothing is kept");
}

#[tokio::test]
async fn stopping_everything_force_kills_whatever_outlived_the_grace_period() {
    let mut harness = harness_with_kill_timeout(0);
    start_one(&mut harness, "web", SLEEPER).await;
    harness
        .daemon
        .handle(DaemonRequest::StopAll)
        .await
        .expect("should stop everything");
    assert!(harness.ports.tracked_pids().await.is_empty());
}

#[tokio::test]
async fn stopping_everything_waits_out_the_grace_period_for_a_child_that_lingers() {
    let mut harness = harness_with_kill_timeout(120);
    adapters::ProcessLauncher::adopt(&*harness.ports, u32::MAX).await;

    harness
        .daemon
        .handle(DaemonRequest::StopAll)
        .await
        .expect("should stop everything");

    assert_eq!(
        harness.ports.tracked_pids().await,
        vec![u32::MAX],
        "an unsignalable pid stays tracked after the sweep"
    );
}
