use adapters::{
    Liveness, ProcessProbe as _, SupervisionReply, SupervisionRequest, service_file_of,
};

use super::{shared::*, test_helpers::*, *};

#[tokio::test]
async fn each_app_expands_the_placeholder_with_its_own_working_directory() {
    let mut harness = harness();
    for name in ["web", "db"] {
        let body = format!(
            "name: {name}\nscript: /bin/sh\nargs:\n  - \"-c\"\n  - \"true\"\n  - \"${{PM3_SERVICE_CWD}}\"\n"
        );
        std::fs::write(
            service_file_of(&harness.cfg_dir, name).expect("a safe service name"),
            body,
        )
        .expect("write the service");
    }
    harness
        .daemon
        .handle(SupervisionRequest::Start {
            services: vec!["web".to_string(), "db".to_string()],
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
async fn starting_a_service_without_a_file_is_refused() {
    let mut harness = harness();
    let outcome = harness
        .daemon
        .handle(SupervisionRequest::Start {
            services: vec!["ghost".to_string()],
        })
        .await;
    assert!(outcome.is_err(), "got: {outcome:?}");
}

#[tokio::test]
async fn starting_no_services_launches_nothing() {
    let mut harness = harness();
    let reply = harness
        .daemon
        .handle(SupervisionRequest::Start {
            services: Vec::new(),
        })
        .await
        .expect("an empty request is not an error");
    assert_eq!(
        reply,
        SupervisionReply::Started {
            outcomes: Vec::new(),
            refused: Vec::new(),
            reason: None,
            unsaved: None,
        }
    );
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
        .handle(SupervisionRequest::Describe(selector("web")))
        .await
        .expect("should describe");
    assert!(
        matches!(reply, SupervisionReply::Described(view) if view.name == "web"),
        "describe should answer with the app"
    );
}

#[tokio::test]
async fn describing_an_unknown_app_is_refused() {
    let mut harness = harness();
    let outcome = harness
        .daemon
        .handle(SupervisionRequest::Describe(selector("ghost")))
        .await;
    assert!(outcome.is_err(), "got: {outcome:?}");
}

#[tokio::test]
async fn stopping_an_app_confirms_its_name() {
    let mut harness = harness();
    start_one(&mut harness, "web", SLEEPER).await;
    let reply = harness
        .daemon
        .handle(SupervisionRequest::Stop(selector("web")))
        .await
        .expect("should stop");
    assert_eq!(
        reply,
        SupervisionReply::Stopped {
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
        .handle(SupervisionRequest::Restart(selector("web")))
        .await
        .expect("should restart");
    assert_eq!(
        reply,
        SupervisionReply::Restarted {
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
        .handle(SupervisionRequest::Delete(selector("web")))
        .await
        .expect("should delete");
    assert_eq!(listed(&mut harness).await, 0);
}

#[tokio::test]
async fn deleting_an_unknown_app_is_refused() {
    let mut harness = harness();
    let outcome = harness
        .daemon
        .handle(SupervisionRequest::Delete(selector("ghost")))
        .await;
    assert!(outcome.is_err(), "got: {outcome:?}");
}

#[tokio::test]
async fn an_app_stopped_on_purpose_settles() {
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
        .handle(SupervisionRequest::Describe(selector("web")))
        .await
        .expect("should describe");
    assert!(
        matches!(reply, SupervisionReply::Described(view) if view.status.as_str() == "stopped"),
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
        .on_exit("web", 0, ExitOutcome::Unobserved)
        .await;
    let reply = harness
        .daemon
        .handle(SupervisionRequest::Describe(selector("web")))
        .await
        .expect("should describe");
    assert!(
        matches!(reply, SupervisionReply::Described(view) if view.status.as_str() == "online"),
        "a stale exit must not touch the current instance"
    );
}

#[tokio::test]
async fn an_exit_for_an_unknown_app_is_tolerated() {
    let mut harness = harness();
    harness
        .daemon
        .on_exit("ghost", 0, ExitOutcome::Unobserved)
        .await;
    assert_eq!(listed(&mut harness).await, 0);
}

#[tokio::test]
async fn restarting_a_stopped_app_launches_it_again() {
    let mut harness = harness();
    start_one(&mut harness, "web", SLEEPER).await;
    harness
        .daemon
        .handle(SupervisionRequest::Stop(selector("web")))
        .await
        .expect("should stop");
    let (name, generation, outcome) = next_exit(&mut harness.events).await;
    harness.daemon.on_exit(&name, generation, outcome).await;

    queue_restart(&mut harness, "web", 0);
    harness.daemon.on_restart("web").await;
    let reply = harness
        .daemon
        .handle(SupervisionRequest::Describe(selector("web")))
        .await
        .expect("should describe");
    assert!(
        matches!(reply, SupervisionReply::Described(view) if view.status.as_str() == "online"),
        "a stopped app should come back online"
    );
}

#[tokio::test]
async fn a_scheduled_restart_of_a_running_app_waits_for_its_exit() {
    let mut harness = harness();
    start_one(&mut harness, "web", SLEEPER).await;
    queue_restart(&mut harness, "web", 0);
    harness.daemon.on_restart("web").await;
    let reply = harness
        .daemon
        .handle(SupervisionRequest::Describe(selector("web")))
        .await
        .expect("should describe");
    assert!(
        matches!(reply, SupervisionReply::Described(view) if view.status.as_str() == "stopping"),
        "a running app must be stopped before it restarts"
    );
}

#[tokio::test]
async fn a_restart_cancelled_by_a_stop_no_longer_revives_the_service() {
    let mut harness = harness();
    start_one(&mut harness, "web", SLEEPER).await;
    queue_restart(&mut harness, "web", 60_000);
    harness
        .daemon
        .handle(SupervisionRequest::Stop(selector("web")))
        .await
        .expect("should stop");

    harness.daemon.on_restart("web").await;

    let reply = harness
        .daemon
        .handle(SupervisionRequest::Describe(selector("web")))
        .await
        .expect("should describe");
    assert!(
        matches!(reply, SupervisionReply::Described(view) if view.status.as_str() != "online"),
        "a service the operator stopped must not come back on a stale restart"
    );
}

#[tokio::test]
async fn restarting_an_unknown_app_is_tolerated() {
    let mut harness = harness();
    queue_restart(&mut harness, "ghost", 0);
    harness.daemon.on_restart("ghost").await;
    assert_eq!(listed(&mut harness).await, 0);
}

#[tokio::test]
async fn a_force_kill_from_an_older_generation_is_ignored() {
    let mut harness = harness();
    let started = start_one(&mut harness, "web", SLEEPER).await;
    let pid = started.pid.expect("a pid");
    harness.daemon.on_force_kill("web", 0, pid, None).await;
    assert_eq!(listed(&mut harness).await, 1);
}

#[tokio::test]
async fn a_force_kill_of_an_untracked_pid_is_ignored() {
    let mut harness = harness();
    start_one(&mut harness, "web", SLEEPER).await;
    harness.daemon.on_force_kill("web", 1, 1, None).await;
    assert_eq!(listed(&mut harness).await, 1);
}

#[tokio::test]
async fn a_force_kill_stops_a_tracked_app() {
    let mut harness = harness();
    let started = start_one(&mut harness, "web", SLEEPER).await;
    let pid = started.pid.expect("a pid");
    harness.daemon.on_force_kill("web", 1, pid, None).await;
    let (_name, _generation, outcome) = next_exit(&mut harness.events).await;
    assert_eq!(outcome, ExitOutcome::Signalled);
}

#[tokio::test]
async fn a_force_kill_with_a_mismatched_token_spares_the_process() {
    let mut harness = harness();
    let started = start_one(&mut harness, "web", SLEEPER).await;
    let pid = started.pid.expect("a pid");
    harness
        .daemon
        .on_force_kill("web", 1, pid, Some("Mon Jan 01 00:00:00 2020"))
        .await;
    assert_eq!(
        status_of(&mut harness, "web").await,
        "online",
        "a pid the kernel handed to another process must not be signalled"
    );
}

#[tokio::test]
async fn a_force_kill_with_the_matching_token_stops_the_app() {
    let mut harness = harness();
    let started = start_one(&mut harness, "web", SLEEPER).await;
    let pid = started.pid.expect("a pid");
    let Liveness::Alive(token) = harness.ports.identity(pid).await else {
        panic!("a live process reports its identity")
    };
    harness
        .daemon
        .on_force_kill("web", 1, pid, Some(&token))
        .await;
    let (_name, _generation, outcome) = next_exit(&mut harness.events).await;
    assert_eq!(outcome, ExitOutcome::Signalled);
}

#[tokio::test]
async fn an_exit_cancels_the_force_kill_that_was_waiting_for_it() {
    let mut harness = harness();
    let started = start_one(&mut harness, "web", SLEEPER).await;
    let pid = started.pid.expect("a pid");
    harness
        .daemon
        .handle(SupervisionRequest::Stop(selector("web")))
        .await
        .expect("should stop");
    assert!(
        harness.daemon.board.has_force_kill("web"),
        "a stop arms the force kill that waits on pid {pid}"
    );

    harness.daemon.on_exit("web", 1, ExitOutcome::Code(0)).await;

    assert!(
        !harness.daemon.board.has_force_kill("web"),
        "a settled exit must cancel the force kill that was waiting on pid {pid}"
    );
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
