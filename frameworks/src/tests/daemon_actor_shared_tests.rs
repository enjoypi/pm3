use std::fmt::Write as _;

use adapters::{StartOutcome, SupervisionReply, SupervisionRequest, service_file_of};

use super::{test_helpers::*, *};

pub async fn start_one(harness: &mut Harness, name: &str, script: &str) -> StartOutcome {
    apps_file(harness, name, script);
    let reply = harness
        .daemon
        .handle(SupervisionRequest::Start {
            services: vec![name.to_string()],
        })
        .await
        .expect("should start");
    let SupervisionReply::Started {
        mut outcomes,
        refused: _,
        reason: _,
        unsaved: _,
    } = reply
    else {
        panic!("start should answer with a start summary")
    };
    outcomes.pop().expect("one app should start")
}
pub async fn next_exit(events: &mut mpsc::Receiver<DaemonEvent>) -> (String, u64, ExitOutcome) {
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
pub async fn next_force_kill(
    events: &mut mpsc::Receiver<DaemonEvent>,
) -> (String, u64, u32, Option<String>) {
    loop {
        let DaemonEvent::ForceKill {
            name,
            generation,
            pid,
            token,
        } = next_event(events).await
        else {
            continue;
        };
        return (name, generation, pid, token);
    }
}
pub async fn listed(harness: &mut Harness) -> usize {
    let reply = harness
        .daemon
        .handle(SupervisionRequest::List)
        .await
        .expect("should list");
    let SupervisionReply::Listed(views) = reply else {
        panic!("list should answer with a table")
    };
    views.len()
}
pub async fn described(harness: &mut Harness, name: &str) -> adapters::ProcessView {
    let reply = harness
        .daemon
        .handle(SupervisionRequest::Describe(selector(name)))
        .await
        .expect("should describe");
    let SupervisionReply::Described(view) = reply else {
        panic!("describe should answer with a view")
    };
    view
}
pub async fn start_scheduled(harness: &mut Harness, name: &str, cron: &str) -> StartOutcome {
    scheduled_apps_file(harness, name, SLEEPER, cron);
    let reply = harness
        .daemon
        .handle(SupervisionRequest::Start {
            services: vec![name.to_string()],
        })
        .await
        .expect("should register the task");
    let SupervisionReply::Started {
        mut outcomes,
        refused: _,
        reason: _,
        unsaved: _,
    } = reply
    else {
        panic!("start should answer with a start summary")
    };
    outcomes.pop().expect("one app should register")
}
pub async fn armed_fire(harness: &mut Harness, name: &str) -> u64 {
    described(harness, name)
        .await
        .next_fire_ms
        .expect("a scheduled task advertises its next fire")
}
pub async fn status_of(harness: &mut Harness, name: &str) -> String {
    let reply = harness
        .daemon
        .handle(SupervisionRequest::Describe(selector(name)))
        .await
        .expect("should describe");
    let SupervisionReply::Described(view) = reply else {
        panic!("describe should answer with a view")
    };
    view.status.as_str().to_string()
}

pub fn service_with_script(harness: &Harness, name: &str, script: &str, depends_on: &[&str]) {
    let cwd = harness.paths.root.to_string_lossy();
    let deps = depends_on
        .iter()
        .fold(String::new(), |mut text, dependency| {
            let _ = writeln!(text, "  - {dependency}");
            text
        });
    let listed = if deps.is_empty() {
        String::new()
    } else {
        format!("depends_on:\n{deps}")
    };
    let service =
        format!("name: {name}\nscript: \"{script}\"\ncwd: \"{cwd}\"\nautorestart: false\n{listed}");
    std::fs::write(
        service_file_of(&harness.cfg_dir, name).expect("a safe service name"),
        service,
    )
    .expect("write the service file");
}

pub fn unrunnable_script(harness: &Harness) -> String {
    let path = harness.dir.path().join("not-executable");
    std::fs::write(&path, "").expect("write a file nobody can execute");
    path.to_string_lossy().into_owned()
}
