use adapters::{StartKind, service_file_of};

use super::{shared::*, test_helpers::*, *};
use crate::daemon::runner::run;

#[tokio::test]
async fn stopping_everything_takes_every_app_down() {
    let mut harness = harness();
    start_one(&mut harness, "web", SLEEPER).await;
    harness
        .daemon
        .handle(SupervisionRequest::StopAll)
        .await
        .expect("should stop everything");
    assert_ne!(status_of(&mut harness, "web").await, "online");
}

#[tokio::test]
async fn stopping_everything_names_what_it_stopped() {
    let mut harness = harness();
    start_one(&mut harness, "web", SLEEPER).await;
    let reply = harness
        .daemon
        .handle(SupervisionRequest::StopAll)
        .await
        .expect("should stop everything");
    assert!(
        matches!(reply, SupervisionReply::StoppedAll { names } if names == vec!["web".to_string()]),
        "stop-all should report the services it stopped"
    );
}

#[tokio::test]
async fn stopping_everything_on_an_empty_table_reports_nothing() {
    let mut harness = harness();
    let reply = harness
        .daemon
        .handle(SupervisionRequest::StopAll)
        .await
        .expect("should stop everything");
    assert!(
        matches!(reply, SupervisionReply::StoppedAll { names } if names.is_empty()),
        "an empty table stops nothing"
    );
}

#[tokio::test]
async fn stopping_everything_reports_a_dump_it_cannot_write() {
    let mut harness = harness();
    std::fs::create_dir_all(&harness.paths.dump_file).expect("block the dump path");
    std::fs::write(harness.paths.dump_file.join("occupied"), "state")
        .expect("fill the blocked dump path");
    let outcome = harness.daemon.handle(SupervisionRequest::StopAll).await;
    assert!(outcome.is_err(), "got: {outcome:?}");
}

#[tokio::test]
async fn shutting_down_leaves_every_app_running() {
    let mut harness = harness();
    start_one(&mut harness, "web", SLEEPER).await;
    harness.daemon.shutdown().await;
    assert_eq!(status_of(&mut harness, "web").await, "online");
}

#[tokio::test]
async fn shutting_down_settles_a_service_that_was_still_stopping() {
    let mut harness = harness_with_kill_timeout(0);
    start_one(&mut harness, "web", SLEEPER).await;
    harness
        .daemon
        .handle(SupervisionRequest::Stop(selector("web")))
        .await
        .expect("should stop");
    assert_eq!(status_of(&mut harness, "web").await, "stopping");

    harness.daemon.shutdown().await;

    assert_eq!(
        status_of(&mut harness, "web").await,
        "stopping",
        "the next daemon must still see that this service was told to stop"
    );
}

#[tokio::test]
async fn a_service_stopped_before_the_handover_is_settled_by_the_next_daemon() {
    let mut harness = harness_with_kill_timeout(0);
    start_one(&mut harness, "web", SLEEPER).await;
    harness
        .daemon
        .handle(SupervisionRequest::Stop(selector("web")))
        .await
        .expect("should stop");
    harness.daemon.shutdown().await;

    harness.daemon.resurrect_saved_apps().await;

    assert_eq!(status_of(&mut harness, "web").await, "stopped");
}

#[tokio::test]
async fn shutting_down_tolerates_a_dump_it_cannot_write() {
    let mut harness = harness_with_kill_timeout(0);
    start_one(&mut harness, "web", SLEEPER).await;
    harness
        .daemon
        .handle(SupervisionRequest::Stop(selector("web")))
        .await
        .expect("should stop");
    std::fs::remove_file(&harness.paths.dump_file).expect("drop the dump file");
    std::fs::create_dir_all(&harness.paths.dump_file).expect("block the dump path");
    std::fs::write(harness.paths.dump_file.join("occupied"), "state")
        .expect("fill the blocked dump path");

    harness.daemon.shutdown().await;

    assert_eq!(status_of(&mut harness, "web").await, "stopping");
}

#[tokio::test]
async fn shutting_down_signals_nothing() {
    let mut harness = harness();
    let started = start_one(&mut harness, "web", SLEEPER).await;
    let pid = started.pid.expect("a pid");
    harness.daemon.shutdown().await;
    assert!(
        harness.ports.tracked_pids().await.contains(&pid),
        "a preserved service must stay tracked, not be signalled"
    );
}

#[tokio::test]
async fn shutting_down_sweeps_a_stray_once_the_drain_budget_expires() {
    let mut harness = harness_with_kill_timeout(60);
    let started = start_one(&mut harness, "web", SLEEPER).await;
    let pid = started.pid.expect("a pid");
    adapters::ProcessLauncher::adopt(&*harness.ports, u32::MAX).await;

    harness.daemon.shutdown().await;

    assert!(
        harness.ports.tracked_pids().await.contains(&pid),
        "the sweep must spare preserved services"
    );
}

#[tokio::test]
async fn stopping_everything_force_kills_a_child_the_table_forgot() {
    let mut harness = harness_with_kill_timeout(0);
    apps_file_without_restart(&harness, "web", SLEEPER);
    let reply = harness
        .daemon
        .handle(SupervisionRequest::Start {
            services: vec!["web".to_string()],
        })
        .await
        .expect("should start");
    let SupervisionReply::Started {
        outcomes,
        refused: _,
        reason: _,
    } = reply
    else {
        panic!("start should answer with a start summary")
    };
    let started = outcomes.first().expect("one app should start");
    let pid = started.pid.expect("a pid");

    harness
        .daemon
        .on_exit("web", 1, ExitOutcome { exit_code: Some(0) })
        .await;
    harness
        .daemon
        .handle(SupervisionRequest::StopAll)
        .await
        .expect("should stop everything");

    let (name, generation, killed, token) = next_force_kill(&mut harness.events).await;
    assert_eq!(killed, pid, "the sweep must target the forgotten child");
    harness
        .daemon
        .on_force_kill(&name, generation, killed, token.as_deref())
        .await;

    let (_name, _generation, outcome) = next_exit(&mut harness.events).await;
    assert_eq!(outcome.exit_code, None, "pid {pid} should be force killed");
}

#[tokio::test]
async fn stopping_everything_answers_while_a_stray_is_still_draining() {
    let Harness {
        dir: _dir,
        paths: _paths,
        cfg_dir: _cfg_dir,
        daemon,
        ports,
        events,
        sender: _sender,
    } = harness_with_kill_timeout(60_000);
    adapters::ProcessLauncher::adopt(&*ports, u32::MAX).await;
    let (commands, command_queue) = mpsc::channel(CHANNEL_DEPTH);
    let supervisor = tokio::spawn(run(daemon, command_queue, events));

    let (stop_all, stopped) = command(SupervisionRequest::StopAll);
    commands.send(stop_all).await.expect("should queue");
    tokio::time::timeout(EVENT_BUDGET, stopped)
        .await
        .expect("stop-all must answer long before the kill timeout expires")
        .expect("should answer")
        .expect("should stop everything");

    let (list, listed) = command(SupervisionRequest::List);
    commands.send(list).await.expect("should queue");
    tokio::time::timeout(EVENT_BUDGET, listed)
        .await
        .expect("the actor must keep serving while a stray drains")
        .expect("should answer")
        .expect("should list");

    supervisor.abort();
}

#[tokio::test]
async fn the_supervisor_answers_commands() {
    let (commands, command_queue) = mpsc::channel(CHANNEL_DEPTH);
    let Harness {
        dir: _dir,
        paths: _paths,
        cfg_dir: _cfg_dir,
        daemon,
        ports: _ports,
        events,
        sender,
    } = harness();
    let supervisor = tokio::spawn(run(daemon, command_queue, events));

    let (command, answer) = command(SupervisionRequest::List);
    commands.send(command).await.expect("should queue");
    let reply = answer.await.expect("should answer").expect("should list");
    assert_eq!(reply, SupervisionReply::Listed(Vec::new()));

    sender
        .send(DaemonEvent::Shutdown)
        .await
        .expect("should queue");
    supervisor.await.expect("join");
}

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
            outcome: ExitOutcome { exit_code: None },
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
    let cwd = harness.paths.root.to_string_lossy().into_owned();
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
    let cwd = harness.paths.root.to_string_lossy().into_owned();
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

#[tokio::test]
async fn shutting_down_counts_only_the_services_still_running() {
    let mut harness = harness();
    start_one(&mut harness, "web", SLEEPER).await;
    start_one(&mut harness, "db", SLEEPER).await;
    harness
        .daemon
        .handle(SupervisionRequest::Stop(selector("db")))
        .await
        .expect("should stop db");
    harness.daemon.shutdown().await;
    assert_eq!(status_of(&mut harness, "web").await, "online");
}

#[path = "daemon_actor_batch_tests.rs"]
mod batch;
