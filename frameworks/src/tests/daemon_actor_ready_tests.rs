use super::*;

fn probing_service(harness: &Harness, name: &str, probe: &str, listen_timeout_ms: u64) {
    let cwd = workspace_of(harness);
    let body = format!(
        "name: {name}\nscript: /bin/sh\ncwd: \"{cwd}\"\nautorestart: true\nlisten_timeout_ms: {listen_timeout_ms}\nready_probe:\n{probe}\nargs:\n  - \"-c\"\n  - \"sleep 30\"\n"
    );
    std::fs::write(
        service_file_of(&harness.cfg_dir, name).expect("a safe service name"),
        body,
    )
    .expect("write the service");
}

fn dependent_service(harness: &Harness, name: &str, depends_on: &str) {
    let cwd = workspace_of(harness);
    let body = format!(
        "name: {name}\nscript: /bin/sh\ncwd: \"{cwd}\"\nautorestart: true\ndepends_on:\n  - {depends_on}\nargs:\n  - \"-c\"\n  - \"sleep 30\"\n"
    );
    std::fs::write(
        service_file_of(&harness.cfg_dir, name).expect("a safe service name"),
        body,
    )
    .expect("write the service");
}

async fn start(harness: &mut Harness, services: &[&str]) -> SupervisionReply {
    harness
        .daemon
        .handle(SupervisionRequest::Start {
            services: services.iter().map(ToString::to_string).collect(),
        })
        .await
        .expect("should start")
}

#[tokio::test]
async fn a_service_with_a_passing_probe_becomes_online() {
    let mut harness = harness();
    probing_service(&harness, "web", "  exec:\n    - \"/usr/bin/true\"", 5000);
    start(&mut harness, &["web"]).await;
    assert_eq!(status_of(&mut harness, "web").await, "launching");

    let event = next_event(&mut harness.events).await;
    assert!(matches!(event, DaemonEvent::Ready { .. }), "got: {event:?}");
    harness.daemon.apply(event).await;

    assert_eq!(status_of(&mut harness, "web").await, "online");
}

#[tokio::test]
async fn a_missing_probe_command_fails_the_service_fast() {
    let mut harness = harness();
    probing_service(
        &harness,
        "web",
        "  exec:\n    - \"/nonexistent/probe\"",
        30000,
    );
    start(&mut harness, &["web"]).await;

    let event = next_event(&mut harness.events).await;
    assert!(
        matches!(event, DaemonEvent::ReadyTimeout { .. }),
        "got: {event:?}"
    );
    harness.daemon.apply(event).await;
    let (name, generation, outcome) = next_exit(&mut harness.events).await;
    harness.daemon.on_exit(&name, generation, outcome).await;

    assert_eq!(status_of(&mut harness, "web").await, "errored");
}

#[tokio::test]
async fn a_service_that_never_answers_is_stopped_after_its_timeout() {
    let mut harness = harness();
    probing_service(&harness, "web", "  exec:\n    - \"/usr/bin/false\"", 300);
    start(&mut harness, &["web"]).await;

    let event = next_event(&mut harness.events).await;
    assert!(
        matches!(event, DaemonEvent::ReadyTimeout { .. }),
        "got: {event:?}"
    );
    harness.daemon.apply(event).await;
    assert_eq!(status_of(&mut harness, "web").await, "stopping");
    let (name, generation, outcome) = next_exit(&mut harness.events).await;
    harness.daemon.on_exit(&name, generation, outcome).await;

    assert_eq!(status_of(&mut harness, "web").await, "errored");
}

#[tokio::test]
async fn a_dependent_service_waits_for_its_dependency_to_become_ready() {
    let mut harness = harness();
    probing_service(&harness, "db", "  exec:\n    - \"/usr/bin/true\"", 5000);
    dependent_service(&harness, "web", "db");
    let reply = start(&mut harness, &["db", "web"]).await;
    let SupervisionReply::Started {
        outcomes, refused, ..
    } = &reply
    else {
        panic!("start should answer with a start summary")
    };
    assert!(refused.is_empty(), "got: {refused:?}");
    assert_eq!(outcomes.len(), 2);
    assert_eq!(status_of(&mut harness, "db").await, "launching");
    assert_eq!(status_of(&mut harness, "web").await, "stopped");

    let event = next_event(&mut harness.events).await;
    harness.daemon.apply(event).await;

    assert_eq!(status_of(&mut harness, "db").await, "online");
    assert_eq!(status_of(&mut harness, "web").await, "online");
}

#[tokio::test]
async fn a_dependency_that_never_becomes_ready_cancels_its_waiter() {
    let mut harness = harness();
    probing_service(&harness, "db", "  exec:\n    - \"/usr/bin/false\"", 300);
    dependent_service(&harness, "web", "db");
    start(&mut harness, &["db", "web"]).await;
    assert_eq!(status_of(&mut harness, "web").await, "stopped");

    let event = next_event(&mut harness.events).await;
    harness.daemon.apply(event).await;

    assert_eq!(status_of(&mut harness, "web").await, "errored");
    let (name, generation, outcome) = next_exit(&mut harness.events).await;
    harness.daemon.on_exit(&name, generation, outcome).await;
    assert_eq!(status_of(&mut harness, "db").await, "errored");
}

#[tokio::test]
async fn stopping_a_service_mid_probe_cancels_the_watch() {
    let mut harness = harness();
    probing_service(&harness, "web", "  exec:\n    - \"/usr/bin/false\"", 30000);
    start(&mut harness, &["web"]).await;
    assert_eq!(status_of(&mut harness, "web").await, "launching");

    harness
        .daemon
        .handle(SupervisionRequest::Stop(selector("web")))
        .await
        .expect("should stop");

    assert_eq!(status_of(&mut harness, "web").await, "stopping");
    let (name, generation, outcome) = next_exit(&mut harness.events).await;
    harness.daemon.on_exit(&name, generation, outcome).await;
    assert_eq!(status_of(&mut harness, "web").await, "stopped");
}

#[tokio::test]
async fn a_resurrected_probe_service_is_adopted_without_rewaiting() {
    let mut origin = harness();
    probing_service(&origin, "web", "  exec:\n    - \"/usr/bin/true\"", 5000);
    start(&mut origin, &["web"]).await;
    let mut revived = harness();
    std::fs::copy(&origin.paths.dump_file, &revived.paths.dump_file).expect("copy the dump");
    std::fs::copy(
        origin.cfg_dir.join("web.yaml"),
        revived.cfg_dir.join("web.yaml"),
    )
    .expect("copy the service file");

    revived.daemon.resurrect_saved_apps().await;

    assert_eq!(status_of(&mut revived, "web").await, "online");
}

#[tokio::test]
async fn a_failed_probe_settlement_survives_a_save_failure() {
    let mut harness = harness();
    probing_service(&harness, "web", "  exec:\n    - \"/usr/bin/false\"", 300);
    start(&mut harness, &["web"]).await;
    let event = next_event(&mut harness.events).await;
    harness.daemon.apply(event).await;
    std::fs::remove_file(&harness.paths.dump_file).expect("drop the dump file");
    std::fs::create_dir(&harness.paths.dump_file).expect("block the dump path");

    let (name, generation, outcome) = next_exit(&mut harness.events).await;
    harness.daemon.on_exit(&name, generation, outcome).await;

    assert_eq!(status_of(&mut harness, "web").await, "errored");
    std::fs::remove_dir(&harness.paths.dump_file).expect("unblock the dump path");
}
