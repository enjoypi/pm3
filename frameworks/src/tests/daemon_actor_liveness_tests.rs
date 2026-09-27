use super::*;

#[tokio::test]
async fn a_liveness_sample_arms_the_following_one() {
    let mut harness = harness();
    start_one(&mut harness, "web", SLEEPER).await;

    harness.daemon.on_liveness_sample().await;

    let event = next_event(&mut harness.events).await;
    assert!(
        matches!(event, DaemonEvent::SampleLiveness),
        "the liveness sampler must keep itself running, got: {event:?}"
    );
    harness.daemon.apply(event).await;
}
