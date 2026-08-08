use super::*;

#[tokio::test]
async fn a_running_service_reports_its_resource_usage() {
    let mut harness = harness();
    start_one(&mut harness, "web", SLEEPER).await;
    let view = described(&mut harness, "web").await;
    let rss = view.rss_kib.expect("a running service has a memory sample");
    assert!(rss > 0, "got: {rss}");
    assert!(
        view.cpu_tenths.is_some(),
        "a running service has a cpu sample"
    );
}

#[tokio::test]
async fn a_stopped_service_has_no_resource_usage() {
    let mut harness = harness();
    start_one(&mut harness, "web", SLEEPER).await;
    harness
        .daemon
        .handle(SupervisionRequest::Stop(selector("web")))
        .await
        .expect("should stop");
    let view = described(&mut harness, "web").await;
    assert_eq!(view.rss_kib, None);
    assert_eq!(view.cpu_tenths, None);
}

#[tokio::test]
async fn the_listing_carries_the_resource_usage_of_running_services() {
    let mut harness = harness();
    start_one(&mut harness, "web", SLEEPER).await;
    let reply = harness
        .daemon
        .handle(SupervisionRequest::List)
        .await
        .expect("should list");
    let SupervisionReply::Listed(views) = reply else {
        panic!("list should answer with a table")
    };
    let web = views
        .iter()
        .find(|view| view.name == "web")
        .expect("web should be listed");
    assert!(web.rss_kib.is_some(), "got: {web:?}");
}
