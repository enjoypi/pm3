use super::*;

#[tokio::test]
async fn the_supervisor_keeps_running_when_the_command_queue_closes() {
    let Harness {
        dir: _dir,
        paths: _paths,
        cfg_dir: _cfg_dir,
        daemon,
        ports: _ports,
        events,
        sender,
    } = harness();
    let (commands, command_queue) = mpsc::channel::<DaemonCommand>(CHANNEL_DEPTH);
    let supervisor = tokio::spawn(run(daemon, command_queue, events));
    drop(commands);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert!(!supervisor.is_finished(), "only a shutdown should stop it");
    sender
        .send(DaemonEvent::Shutdown)
        .await
        .expect("should queue");
    supervisor.await.expect("join");
}

#[tokio::test]
async fn the_supervisor_stops_on_a_shutdown_event() {
    let Harness {
        dir: _dir,
        paths: _paths,
        cfg_dir: _cfg_dir,
        daemon,
        ports: _ports,
        events,
        sender,
    } = harness();
    let (_commands, command_queue) = mpsc::channel::<DaemonCommand>(CHANNEL_DEPTH);
    let supervisor = tokio::spawn(run(daemon, command_queue, events));
    sender
        .send(DaemonEvent::Shutdown)
        .await
        .expect("should queue");
    supervisor.await.expect("join");
}

#[tokio::test]
async fn the_supervisor_handles_internal_events() {
    let Harness {
        dir: _dir,
        paths: _paths,
        cfg_dir: _cfg_dir,
        daemon,
        ports: _ports,
        events,
        sender,
    } = harness();
    let (_commands, command_queue) = mpsc::channel::<DaemonCommand>(CHANNEL_DEPTH);
    let supervisor = tokio::spawn(run(daemon, command_queue, events));
    sender
        .send(DaemonEvent::Restart {
            name: "ghost".to_string(),
        })
        .await
        .expect("should queue");
    sender
        .send(DaemonEvent::Exited {
            name: "ghost".to_string(),
            generation: 0,
            outcome: ExitOutcome::Unobserved,
        })
        .await
        .expect("should queue");
    sender
        .send(DaemonEvent::ForceKill {
            name: "ghost".to_string(),
            generation: 7,
            pid: 1,
            token: None,
        })
        .await
        .expect("should queue");
    sender
        .send(DaemonEvent::Fire {
            name: "ghost".to_string(),
            fire_at_ms: 1,
        })
        .await
        .expect("should queue");
    sender
        .send(DaemonEvent::Shutdown)
        .await
        .expect("should queue");
    supervisor.await.expect("join");
}

#[tokio::test]
async fn a_confined_app_is_refused_when_no_sandbox_backend_exists() {
    let mut harness = harness();
    let cwd = workspace_of(&harness);
    let body = format!(
        "name: web\nscript: /bin/sh\ncwd: \"{cwd}\"\nargs:\n  - \"-c\"\n  - \"sleep 30\"\nsandbox:\n  mode: workspace-write\n"
    );
    std::fs::write(
        service_file_of(&harness.cfg_dir, "web").expect("a safe service name"),
        &body,
    )
    .expect("write the service");
    let err = harness
        .daemon
        .handle(SupervisionRequest::Start {
            services: vec!["web".to_string()],
        })
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("no usable sandbox backend"), "got: {err}");
}

#[tokio::test]
async fn starting_an_already_running_app_leaves_it_alone() {
    let mut harness = harness();
    apps_file(&harness, "web", SLEEPER);
    let request = SupervisionRequest::Start {
        services: vec!["web".to_string()],
    };
    harness
        .daemon
        .handle(request.clone())
        .await
        .expect("first start");
    let reply = harness.daemon.handle(request).await.expect("second start");
    let SupervisionReply::Started {
        outcomes,
        refused: _,
        reason: _,
        unsaved: _,
    } = reply
    else {
        panic!("start should answer with a start summary")
    };
    assert!(
        outcomes
            .iter()
            .all(|outcome| outcome.kind == StartKind::AlreadyRunning),
        "the second start should report the app as already running"
    );
}

#[tokio::test]
async fn a_writable_root_that_does_not_exist_is_kept_verbatim() {
    let mut harness = harness();
    let cwd = workspace_of(&harness);
    let body = format!(
        "name: web\nscript: /bin/sh\ncwd: \"{cwd}\"\nargs:\n  - \"-c\"\n  - \"sleep 30\"\nsandbox:\n  mode: workspace-write\n  writable_roots:\n    - /nonexistent/pm3-root\n"
    );
    std::fs::write(
        service_file_of(&harness.cfg_dir, "web").expect("a safe service name"),
        &body,
    )
    .expect("write the service");
    let err = harness
        .daemon
        .handle(SupervisionRequest::Start {
            services: vec!["web".to_string()],
        })
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("no usable sandbox backend"), "got: {err}");
}

#[tokio::test]
async fn an_unusable_default_sandbox_mode_is_refused() {
    let mut harness = harness_with_sandbox_mode("yolo");
    apps_file(&harness, "web", SLEEPER);
    let err = harness
        .daemon
        .handle(SupervisionRequest::Start {
            services: vec!["web".to_string()],
        })
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("sandbox mode 'yolo'"), "got: {err}");
}

#[tokio::test]
async fn stopping_an_unknown_app_is_refused() {
    let mut harness = harness();
    let outcome = harness
        .daemon
        .handle(SupervisionRequest::Stop(selector("ghost")))
        .await;
    assert!(outcome.is_err(), "got: {outcome:?}");
}

#[tokio::test]
async fn restarting_an_unknown_app_through_a_command_is_refused() {
    let mut harness = harness();
    let outcome = harness
        .daemon
        .handle(SupervisionRequest::Restart(selector("ghost")))
        .await;
    assert!(outcome.is_err(), "got: {outcome:?}");
}

#[tokio::test]
async fn restarting_an_app_reads_its_declaration_again() {
    let mut harness = harness();
    start_one(&mut harness, "web", SLEEPER).await;
    std::fs::write(
        adapters::service_file_of(&harness.cfg_dir, "web").expect("a safe service name"),
        "name: web\nscript: /bin/sh\nenv:\n  TUNNEL_TOKEN: \"eyJhIjoiZjQ2\"\n",
    )
    .expect("rewrite the service file");
    let refused = harness
        .daemon
        .handle(SupervisionRequest::Restart(selector("web")))
        .await
        .expect_err("a restart must read the declaration from disk again")
        .to_string();
    assert!(refused.contains("'web.env'"), "{refused}");
}

#[tokio::test]
async fn restarting_a_stopped_app_through_a_command_starts_it_again() {
    let mut harness = harness();
    start_one(&mut harness, "web", SLEEPER).await;
    harness
        .daemon
        .handle(SupervisionRequest::Stop(selector("web")))
        .await
        .expect("should stop");
    let (name, generation, outcome) = next_exit(&mut harness.events).await;
    harness.daemon.on_exit(&name, generation, outcome).await;

    let reply = harness
        .daemon
        .handle(SupervisionRequest::Restart(selector("web")))
        .await
        .expect("should restart");
    assert_eq!(
        reply,
        SupervisionReply::Restarted {
            name: "web".to_string(),
        }
    );
    let described = harness
        .daemon
        .handle(SupervisionRequest::Describe(selector("web")))
        .await
        .expect("should describe");
    assert!(
        matches!(described, SupervisionReply::Described(view) if view.status.as_str() == "online"),
        "the app should be online again"
    );
}

#[path = "daemon_actor_batch_tests.rs"]
mod batch;
#[path = "daemon_actor_memory_tests.rs"]
mod memory;
#[path = "daemon_actor_ready_tests.rs"]
mod ready;
#[path = "daemon_actor_resource_tests.rs"]
mod resource;
#[path = "daemon_actor_rotate_tests.rs"]
mod rotate;
