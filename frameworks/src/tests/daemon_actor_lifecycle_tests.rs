use adapters::{StartKind, SupervisionReply, SupervisionRequest, service_file_of};

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
async fn stopping_everything_survives_a_dump_it_cannot_write() {
    let mut harness = harness();
    start_one(&mut harness, "web", SLEEPER).await;
    std::fs::remove_file(&harness.paths.dump_file).expect("drop the dump file");
    std::fs::create_dir(&harness.paths.dump_file).expect("block the dump path");

    let outcome = harness.daemon.handle(SupervisionRequest::StopAll).await;

    let SupervisionReply::StoppedAll { names } =
        outcome.expect("a dump failure must not undo the stops already signalled")
    else {
        panic!("stop all should answer with the stopped names")
    };
    assert_eq!(names, vec!["web".to_string()]);
    assert_eq!(status_of(&mut harness, "web").await, "stopping");
    std::fs::remove_dir(&harness.paths.dump_file).expect("unblock the dump path");
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
        unsaved: _,
    } = reply
    else {
        panic!("start should answer with a start summary")
    };
    let started = outcomes.first().expect("one app should start");
    let pid = started.pid.expect("a pid");

    harness.daemon.on_exit("web", 1, ExitOutcome::Code(0)).await;
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
    assert_eq!(
        outcome,
        ExitOutcome::Signalled,
        "pid {pid} should be force killed"
    );
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

#[path = "daemon_actor_handover_tests.rs"]
mod handover;
