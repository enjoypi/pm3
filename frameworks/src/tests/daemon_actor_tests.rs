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
async fn resetting_an_app_clears_its_restart_counters() {
    let mut harness = harness();
    start_one(&mut harness, "web", CRASHER).await;
    let (name, generation, outcome) = next_exit(&mut harness.events).await;
    harness.daemon.on_exit(&name, generation, outcome).await;
    let before = described(&mut harness, "web").await;
    assert_eq!(before.restart_time, 1, "got: {before:?}");
    let reply = harness
        .daemon
        .handle(SupervisionRequest::Reset(selector("web")))
        .await
        .expect("should reset");
    assert_eq!(
        reply,
        SupervisionReply::Reset {
            name: "web".to_string(),
        }
    );
    let after = described(&mut harness, "web").await;
    assert_eq!(after.restart_time, 0, "got: {after:?}");
}

#[tokio::test]
async fn resetting_an_unknown_app_reports_not_found() {
    let mut harness = harness();
    let outcome = harness
        .daemon
        .handle(SupervisionRequest::Reset(selector("ghost")))
        .await;
    assert!(outcome.is_err(), "got: {outcome:?}");
}

#[tokio::test]
async fn signalling_a_running_app_confirms_the_delivery() {
    let mut harness = harness();
    start_one(&mut harness, "web", SLEEPER).await;
    let reply = harness
        .daemon
        .handle(SupervisionRequest::Signal {
            selector: selector("web"),
            signal: "USR1".to_string(),
        })
        .await
        .expect("should signal");
    assert_eq!(
        reply,
        SupervisionReply::Signalled {
            name: "web".to_string(),
            signal: "USR1".to_string(),
        }
    );
}

#[tokio::test]
async fn signalling_an_unknown_app_reports_not_found() {
    let mut harness = harness();
    let outcome = harness
        .daemon
        .handle(SupervisionRequest::Signal {
            selector: selector("ghost"),
            signal: "USR1".to_string(),
        })
        .await;
    assert!(outcome.is_err(), "got: {outcome:?}");
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

#[path = "daemon_actor_signal_tests.rs"]
mod signals;
