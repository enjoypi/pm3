use super::*;

#[tokio::test]
async fn a_service_over_its_memory_limit_is_restarted() {
    let mut harness = harness();
    capped_apps_file(&harness, "hog", SLEEPER, "1K");
    harness
        .daemon
        .handle(SupervisionRequest::Start {
            services: vec!["hog".to_string()],
        })
        .await
        .expect("should start");
    described(&mut harness, "hog")
        .await
        .pid
        .expect("the service should be running");

    harness.daemon.on_memory_sample().await;

    assert_eq!(
        status_of(&mut harness, "hog").await,
        "stopping",
        "a breach must take the process down before it comes back"
    );
}

#[tokio::test]
async fn a_service_inside_its_memory_limit_keeps_running() {
    let mut harness = harness();
    capped_apps_file(&harness, "tidy", SLEEPER, "8G");
    harness
        .daemon
        .handle(SupervisionRequest::Start {
            services: vec!["tidy".to_string()],
        })
        .await
        .expect("should start");
    let first = described(&mut harness, "tidy")
        .await
        .pid
        .expect("the service should be running");

    harness.daemon.on_memory_sample().await;

    assert_eq!(described(&mut harness, "tidy").await.pid, Some(first));
}

#[tokio::test]
async fn a_sample_without_any_capped_service_touches_nothing() {
    let mut harness = harness();
    start_one(&mut harness, "plain", SLEEPER).await;
    let first = described(&mut harness, "plain")
        .await
        .pid
        .expect("the service should be running");

    harness.daemon.on_memory_sample().await;

    assert_eq!(described(&mut harness, "plain").await.pid, Some(first));
}

#[tokio::test]
async fn a_sample_arms_the_following_one() {
    let mut harness = harness();
    capped_apps_file(&harness, "hog", SLEEPER, "8G");
    harness
        .daemon
        .handle(SupervisionRequest::Start {
            services: vec!["hog".to_string()],
        })
        .await
        .expect("should start");

    harness.daemon.on_memory_sample().await;

    let event = next_event(&mut harness.events).await;
    assert!(
        matches!(event, DaemonEvent::SampleMemory),
        "the sampler must keep itself running, got: {event:?}"
    );
    harness.daemon.apply(event).await;
}
