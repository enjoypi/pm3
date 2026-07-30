use adapters::StartKind;

use super::{test_helpers::*, *};

async fn start_one(harness: &mut Harness, name: &str, script: &str) -> StartOutcome {
    let file = apps_file(harness, name, script);
    let reply = harness
        .daemon
        .handle(DaemonRequest::Start {
            apps_file: text(&file),
        })
        .await
        .expect("should start");
    let DaemonReply::Started(mut outcomes) = reply else {
        panic!("start should answer with a start summary")
    };
    outcomes.pop().expect("one app should start")
}

async fn next_exit(events: &mut mpsc::Receiver<DaemonEvent>) -> (String, u64, ExitOutcome) {
    let DaemonEvent::Exited {
        name,
        generation,
        outcome,
    } = next_event(events).await
    else {
        panic!("the watcher should report an exit")
    };
    (name, generation, outcome)
}

async fn listed(harness: &mut Harness) -> usize {
    let reply = harness
        .daemon
        .handle(DaemonRequest::List)
        .await
        .expect("should list");
    let DaemonReply::Listed(views) = reply else {
        panic!("list should answer with a table")
    };
    views.len()
}

async fn described(harness: &mut Harness, name: &str) -> adapters::ProcessView {
    let reply = harness
        .daemon
        .handle(DaemonRequest::Describe(selector(name)))
        .await
        .expect("should describe");
    let DaemonReply::Described(view) = reply else {
        panic!("describe should answer with a view")
    };
    view
}

#[tokio::test]
async fn each_app_expands_the_placeholder_with_its_own_working_directory() {
    let mut harness = harness();
    let body = "apps:\n  - name: web\n    script: /bin/sh\n    args:\n      - \"-c\"\n      - \"true\"\n      - \"${PM3_SVC_CWD}\"\n  - name: db\n    script: /bin/sh\n    args:\n      - \"-c\"\n      - \"true\"\n      - \"${PM3_SVC_CWD}\"\n";
    let file = crate::test_support::write_apps_file(harness.dir.path(), body);
    harness
        .daemon
        .handle(DaemonRequest::Start {
            apps_file: text(&file),
        })
        .await
        .expect("should start");
    let web = described(&mut harness, "web").await;
    let db = described(&mut harness, "db").await;
    assert_eq!(web.args.last(), Some(&web.cwd));
    assert_eq!(db.args.last(), Some(&db.cwd));
    assert_ne!(web.cwd, db.cwd);
}

#[tokio::test]
async fn starting_an_apps_file_launches_every_app() {
    let mut harness = harness();
    let started = start_one(&mut harness, "web", SLEEPER).await;
    assert!(started.pid.is_some(), "got: {started:?}");
}

#[tokio::test]
async fn starting_a_missing_apps_file_is_refused() {
    let mut harness = harness();
    let outcome = harness
        .daemon
        .handle(DaemonRequest::Start {
            apps_file: "/nonexistent/apps.yaml".to_string(),
        })
        .await;
    assert!(outcome.is_err(), "got: {outcome:?}");
}

#[tokio::test]
async fn starting_an_apps_file_without_apps_is_refused() {
    let mut harness = harness();
    let file = crate::test_support::write_apps_file(harness.dir.path(), "apps: []\n");
    let outcome = harness
        .daemon
        .handle(DaemonRequest::Start {
            apps_file: text(&file),
        })
        .await;
    assert!(outcome.is_err(), "got: {outcome:?}");
}

#[tokio::test]
async fn listing_reports_every_managed_app() {
    let mut harness = harness();
    start_one(&mut harness, "web", SLEEPER).await;
    assert_eq!(listed(&mut harness).await, 1);
}

#[tokio::test]
async fn describing_reports_one_app() {
    let mut harness = harness();
    start_one(&mut harness, "web", SLEEPER).await;
    let reply = harness
        .daemon
        .handle(DaemonRequest::Describe(selector("web")))
        .await
        .expect("should describe");
    assert!(
        matches!(reply, DaemonReply::Described(view) if view.name == "web"),
        "describe should answer with the app"
    );
}

#[tokio::test]
async fn describing_an_unknown_app_is_refused() {
    let mut harness = harness();
    let outcome = harness
        .daemon
        .handle(DaemonRequest::Describe(selector("ghost")))
        .await;
    assert!(outcome.is_err(), "got: {outcome:?}");
}

#[tokio::test]
async fn stopping_an_app_confirms_its_name() {
    let mut harness = harness();
    start_one(&mut harness, "web", SLEEPER).await;
    let reply = harness
        .daemon
        .handle(DaemonRequest::Stop(selector("web")))
        .await
        .expect("should stop");
    assert_eq!(
        reply,
        DaemonReply::Stopped {
            name: "web".to_string(),
        }
    );
}

#[tokio::test]
async fn restarting_a_running_app_waits_for_its_exit() {
    let mut harness = harness();
    start_one(&mut harness, "web", SLEEPER).await;
    let reply = harness
        .daemon
        .handle(DaemonRequest::Restart(selector("web")))
        .await
        .expect("should restart");
    assert_eq!(
        reply,
        DaemonReply::Restarted {
            name: "web".to_string(),
        }
    );
}

#[tokio::test]
async fn deleting_an_app_forgets_it() {
    let mut harness = harness();
    start_one(&mut harness, "web", SLEEPER).await;
    harness
        .daemon
        .handle(DaemonRequest::Delete(selector("web")))
        .await
        .expect("should delete");
    assert_eq!(listed(&mut harness).await, 0);
}

#[tokio::test]
async fn deleting_an_unknown_app_is_refused() {
    let mut harness = harness();
    let outcome = harness
        .daemon
        .handle(DaemonRequest::Delete(selector("ghost")))
        .await;
    assert!(outcome.is_err(), "got: {outcome:?}");
}

#[tokio::test]
async fn an_app_stopped_on_purpose_settles() {
    let mut harness = harness();
    start_one(&mut harness, "web", SLEEPER).await;
    harness
        .daemon
        .handle(DaemonRequest::Stop(selector("web")))
        .await
        .expect("should stop");
    let (name, generation, outcome) = next_exit(&mut harness.events).await;
    harness.daemon.on_exit(&name, generation, outcome).await;
    let reply = harness
        .daemon
        .handle(DaemonRequest::Describe(selector("web")))
        .await
        .expect("should describe");
    assert!(
        matches!(reply, DaemonReply::Described(view) if view.status.as_str() == "stopped"),
        "a deliberate stop should settle the app"
    );
}

#[tokio::test]
async fn a_crashing_app_is_scheduled_for_a_restart() {
    let mut harness = harness();
    start_one(&mut harness, "web", CRASHER).await;
    let (name, generation, outcome) = next_exit(&mut harness.events).await;
    harness.daemon.on_exit(&name, generation, outcome).await;
    let queued = next_event(&mut harness.events).await;
    assert!(
        matches!(queued, DaemonEvent::Restart { name: queued_app } if queued_app == "web"),
        "a crash should queue a restart"
    );
}

#[tokio::test]
async fn an_exit_from_an_older_generation_is_ignored() {
    let mut harness = harness();
    start_one(&mut harness, "web", SLEEPER).await;
    harness
        .daemon
        .on_exit("web", 0, ExitOutcome { exit_code: None })
        .await;
    let reply = harness
        .daemon
        .handle(DaemonRequest::Describe(selector("web")))
        .await
        .expect("should describe");
    assert!(
        matches!(reply, DaemonReply::Described(view) if view.status.as_str() == "online"),
        "a stale exit must not touch the current instance"
    );
}

#[tokio::test]
async fn an_exit_for_an_unknown_app_is_tolerated() {
    let mut harness = harness();
    harness
        .daemon
        .on_exit("ghost", 0, ExitOutcome { exit_code: None })
        .await;
    assert_eq!(listed(&mut harness).await, 0);
}

#[tokio::test]
async fn restarting_a_stopped_app_launches_it_again() {
    let mut harness = harness();
    start_one(&mut harness, "web", SLEEPER).await;
    harness
        .daemon
        .handle(DaemonRequest::Stop(selector("web")))
        .await
        .expect("should stop");
    let (name, generation, outcome) = next_exit(&mut harness.events).await;
    harness.daemon.on_exit(&name, generation, outcome).await;

    harness.daemon.on_restart("web").await;
    let reply = harness
        .daemon
        .handle(DaemonRequest::Describe(selector("web")))
        .await
        .expect("should describe");
    assert!(
        matches!(reply, DaemonReply::Described(view) if view.status.as_str() == "online"),
        "a stopped app should come back online"
    );
}

#[tokio::test]
async fn restarting_a_running_app_through_an_event_waits_for_its_exit() {
    let mut harness = harness();
    start_one(&mut harness, "web", SLEEPER).await;
    harness.daemon.on_restart("web").await;
    let reply = harness
        .daemon
        .handle(DaemonRequest::Describe(selector("web")))
        .await
        .expect("should describe");
    assert!(
        matches!(reply, DaemonReply::Described(view) if view.status.as_str() == "stopping"),
        "a running app must be stopped before it restarts"
    );
}

#[tokio::test]
async fn restarting_an_unknown_app_is_tolerated() {
    let mut harness = harness();
    harness.daemon.on_restart("ghost").await;
    assert_eq!(listed(&mut harness).await, 0);
}

#[tokio::test]
async fn a_force_kill_from_an_older_generation_is_ignored() {
    let mut harness = harness();
    let started = start_one(&mut harness, "web", SLEEPER).await;
    let pid = started.pid.expect("a pid");
    harness.daemon.on_force_kill("web", 0, pid).await;
    assert_eq!(listed(&mut harness).await, 1);
}

#[tokio::test]
async fn a_force_kill_of_an_untracked_pid_is_ignored() {
    let mut harness = harness();
    start_one(&mut harness, "web", SLEEPER).await;
    harness.daemon.on_force_kill("web", 1, 1).await;
    assert_eq!(listed(&mut harness).await, 1);
}

#[tokio::test]
async fn a_force_kill_stops_a_tracked_app() {
    let mut harness = harness();
    let started = start_one(&mut harness, "web", SLEEPER).await;
    let pid = started.pid.expect("a pid");
    harness.daemon.on_force_kill("web", 1, pid).await;
    let (_name, _generation, outcome) = next_exit(&mut harness.events).await;
    assert_eq!(outcome.exit_code, None);
}

#[tokio::test]
async fn resurrecting_restores_the_saved_apps() {
    let mut origin = harness();
    start_one(&mut origin, "web", SLEEPER).await;
    let mut revived = harness();
    std::fs::copy(&origin.paths.dump_file, &revived.paths.dump_file).expect("copy the dump");
    std::fs::copy(
        origin.cfg_dir.join("web.yaml"),
        revived.cfg_dir.join("web.yaml"),
    )
    .expect("copy the service file");
    revived.daemon.resurrect_saved_apps().await;
    assert_eq!(listed(&mut revived).await, 1);
}

#[tokio::test]
async fn resurrecting_skips_an_app_without_a_service_file() {
    let mut origin = harness();
    start_one(&mut origin, "web", SLEEPER).await;
    let mut revived = harness();
    std::fs::copy(&origin.paths.dump_file, &revived.paths.dump_file).expect("copy the dump");
    revived.daemon.resurrect_saved_apps().await;
    assert_eq!(listed(&mut revived).await, 0);
}

#[tokio::test]
async fn resurrecting_a_broken_dump_is_tolerated() {
    let mut harness = harness();
    std::fs::write(&harness.paths.dump_file, "{{not yaml").expect("write a broken dump");
    harness.daemon.resurrect_saved_apps().await;
    assert_eq!(listed(&mut harness).await, 0);
}

async fn status_of(harness: &mut Harness, name: &str) -> String {
    let reply = harness
        .daemon
        .handle(DaemonRequest::Describe(selector(name)))
        .await
        .expect("should describe");
    let DaemonReply::Described(view) = reply else {
        panic!("describe should answer with a view")
    };
    view.status.as_str().to_string()
}

#[tokio::test]
async fn stopping_everything_takes_every_app_down() {
    let mut harness = harness();
    start_one(&mut harness, "web", SLEEPER).await;
    harness
        .daemon
        .handle(DaemonRequest::StopAll)
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
        .handle(DaemonRequest::StopAll)
        .await
        .expect("should stop everything");
    assert!(
        matches!(reply, DaemonReply::StoppedAll { names } if names == vec!["web".to_string()]),
        "stop-all should report the services it stopped"
    );
}

#[tokio::test]
async fn stopping_everything_on_an_empty_table_reports_nothing() {
    let mut harness = harness();
    let reply = harness
        .daemon
        .handle(DaemonRequest::StopAll)
        .await
        .expect("should stop everything");
    assert!(
        matches!(reply, DaemonReply::StoppedAll { names } if names.is_empty()),
        "an empty table stops nothing"
    );
}

#[tokio::test]
async fn stopping_everything_reports_a_dump_it_cannot_write() {
    let mut harness = harness();
    std::fs::create_dir_all(&harness.paths.dump_file).expect("block the dump path");
    std::fs::write(harness.paths.dump_file.join("occupied"), "state")
        .expect("fill the blocked dump path");
    let outcome = harness.daemon.handle(DaemonRequest::StopAll).await;
    assert!(outcome.is_err(), "got: {outcome:?}");
}

#[tokio::test]
async fn shutting_down_leaves_every_app_running() {
    let mut harness = harness();
    start_one(&mut harness, "web", SLEEPER).await;
    harness.daemon.shutdown();
    assert_eq!(status_of(&mut harness, "web").await, "online");
}

#[tokio::test]
async fn shutting_down_signals_nothing() {
    let mut harness = harness();
    let started = start_one(&mut harness, "web", SLEEPER).await;
    let pid = started.pid.expect("a pid");
    harness.daemon.shutdown();
    assert!(
        harness.daemon.tracked_pids().await.contains(&pid),
        "a preserved service must stay tracked, not be signalled"
    );
}

#[tokio::test]
async fn stopping_everything_force_kills_a_child_the_table_forgot() {
    let mut harness = harness_with_kill_timeout(0);
    let file = apps_file_without_restart(&harness, "web", SLEEPER);
    let reply = harness
        .daemon
        .handle(DaemonRequest::Start {
            apps_file: text(&file),
        })
        .await
        .expect("should start");
    let DaemonReply::Started(outcomes) = reply else {
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
        .handle(DaemonRequest::StopAll)
        .await
        .expect("should stop everything");

    let (_name, _generation, outcome) = next_exit(&mut harness.events).await;
    assert_eq!(outcome.exit_code, None, "pid {pid} should be force killed");
}

#[tokio::test]
async fn the_supervisor_answers_commands() {
    let (commands, command_queue) = mpsc::channel(CHANNEL_DEPTH);
    let Harness {
        dir: _dir,
        paths: _paths,
        cfg_dir: _cfg_dir,
        daemon,
        events,
        sender,
    } = harness();
    let supervisor = tokio::spawn(run(daemon, command_queue, events));

    let (command, answer) = command(DaemonRequest::List);
    commands.send(command).await.expect("should queue");
    let reply = answer.await.expect("should answer").expect("should list");
    assert_eq!(reply, DaemonReply::Listed(Vec::new()));

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
        "apps:\n  - name: web\n    script: /bin/sh\n    cwd: \"{cwd}\"\n    args:\n      - \"-c\"\n      - \"sleep 30\"\n    sandbox:\n      mode: workspace-write\n"
    );
    let file = crate::test_support::write_apps_file(harness.dir.path(), &body);
    let err = harness
        .daemon
        .handle(DaemonRequest::Start {
            apps_file: text(&file),
        })
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("no usable sandbox backend"), "got: {err}");
}

#[tokio::test]
async fn starting_an_already_running_app_leaves_it_alone() {
    let mut harness = harness();
    let file = apps_file(&harness, "web", SLEEPER);
    let request = DaemonRequest::Start {
        apps_file: text(&file),
    };
    harness
        .daemon
        .handle(request.clone())
        .await
        .expect("first start");
    let reply = harness.daemon.handle(request).await.expect("second start");
    let DaemonReply::Started(outcomes) = reply else {
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
        "apps:\n  - name: web\n    script: /bin/sh\n    cwd: \"{cwd}\"\n    args:\n      - \"-c\"\n      - \"sleep 30\"\n    sandbox:\n      mode: workspace-write\n      writable_roots:\n        - /nonexistent/pm3-root\n"
    );
    let file = crate::test_support::write_apps_file(harness.dir.path(), &body);
    let err = harness
        .daemon
        .handle(DaemonRequest::Start {
            apps_file: text(&file),
        })
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("no usable sandbox backend"), "got: {err}");
}

#[tokio::test]
async fn an_unusable_default_sandbox_mode_is_refused() {
    let mut harness = harness_with_sandbox_mode("yolo");
    let file = apps_file(&harness, "web", SLEEPER);
    let err = harness
        .daemon
        .handle(DaemonRequest::Start {
            apps_file: text(&file),
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
        .handle(DaemonRequest::Stop(selector("ghost")))
        .await;
    assert!(outcome.is_err(), "got: {outcome:?}");
}

#[tokio::test]
async fn restarting_an_unknown_app_through_a_command_is_refused() {
    let mut harness = harness();
    let outcome = harness
        .daemon
        .handle(DaemonRequest::Restart(selector("ghost")))
        .await;
    assert!(outcome.is_err(), "got: {outcome:?}");
}

#[tokio::test]
async fn restarting_a_stopped_app_through_a_command_starts_it_again() {
    let mut harness = harness();
    start_one(&mut harness, "web", SLEEPER).await;
    harness
        .daemon
        .handle(DaemonRequest::Stop(selector("web")))
        .await
        .expect("should stop");
    let (name, generation, outcome) = next_exit(&mut harness.events).await;
    harness.daemon.on_exit(&name, generation, outcome).await;

    let reply = harness
        .daemon
        .handle(DaemonRequest::Restart(selector("web")))
        .await
        .expect("should restart");
    assert_eq!(
        reply,
        DaemonReply::Restarted {
            name: "web".to_string(),
        }
    );
    let described = harness
        .daemon
        .handle(DaemonRequest::Describe(selector("web")))
        .await
        .expect("should describe");
    assert!(
        matches!(described, DaemonReply::Described(view) if view.status.as_str() == "online"),
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
        .handle(DaemonRequest::Stop(selector("db")))
        .await
        .expect("should stop db");
    harness.daemon.shutdown();
    assert_eq!(status_of(&mut harness, "web").await, "online");
}
