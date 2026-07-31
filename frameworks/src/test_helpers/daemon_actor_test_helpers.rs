use std::{fmt::Write as _, path::PathBuf, time::Duration};

use adapters::{
    DaemonCommand, DaemonOutcome, Pm3Paths, logs_dir_of, resolve_paths, service_file_of,
};
use tokio::sync::oneshot;

use super::*;
use crate::test_support::{SANDBOX_MODE, pm3_config_with_home, write_apps_file};

pub const CHANNEL_DEPTH: usize = 16;
pub const EVENT_BUDGET: Duration = Duration::from_secs(5);
pub const SLEEPER: &str = "sleep 30";
pub const CRASHER: &str = "exit 1";

pub struct Harness {
    pub dir: tempfile::TempDir,
    pub paths: Pm3Paths,
    pub cfg_dir: PathBuf,
    pub daemon: Daemon,
    pub events: mpsc::Receiver<DaemonEvent>,
    pub sender: mpsc::Sender<DaemonEvent>,
}

pub fn harness() -> Harness {
    harness_with_kill_timeout(pm3_config_with_home("/unused").kill_timeout_ms)
}

pub fn harness_with_sandbox_mode(mode: &str) -> Harness {
    built_harness(pm3_config_with_home("/unused").kill_timeout_ms, mode)
}

pub fn harness_with_kill_timeout(kill_timeout_ms: u64) -> Harness {
    built_harness(kill_timeout_ms, SANDBOX_MODE)
}

fn built_harness(kill_timeout_ms: u64, sandbox_mode: &str) -> Harness {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = resolve_paths(dir.path());
    std::fs::create_dir_all(&paths.logs_dir).expect("create the log directory");
    let cfg_dir = dir.path().join("svc");
    std::fs::create_dir_all(&cfg_dir).expect("create the service directory");
    let mut config = pm3_config_with_home(&paths.root.to_string_lossy());
    config.kill_timeout_ms = kill_timeout_ms;
    config.sandbox.mode = sandbox_mode.to_string();
    let specs = SpecSource {
        cfg_dir: cfg_dir.clone(),
        config,
        home_dir: paths.root.to_string_lossy().into_owned(),
        logs_dir: logs_dir_of(&paths.root),
        tmp_dir: None,
    };
    let ports = Arc::new(DaemonPorts::new(
        paths.dump_file.clone(),
        specs.clone(),
        None,
    ));
    let (sender, events) = mpsc::channel(CHANNEL_DEPTH);
    let daemon = Daemon::new(specs, ports, sender.clone());
    Harness {
        dir,
        paths,
        cfg_dir,
        daemon,
        events,
        sender,
    }
}

pub fn apps_file(harness: &Harness, name: &str, script: &str) -> PathBuf {
    written_apps_file(harness, name, script, true)
}

pub fn apps_file_without_restart(harness: &Harness, name: &str, script: &str) -> PathBuf {
    written_apps_file(harness, name, script, false)
}

pub fn scheduled_apps_file(harness: &Harness, name: &str, script: &str, cron: &str) -> PathBuf {
    let cwd = harness.paths.root.to_string_lossy();
    let fields = format!(
        "script: /bin/sh\ncwd: \"{cwd}\"\nautorestart: false\nschedule: \"{cron}\"\nargs:\n  - \"-c\"\n  - \"{script}\"\n"
    );
    write_both(harness, name, &fields)
}

fn written_apps_file(harness: &Harness, name: &str, script: &str, autorestart: bool) -> PathBuf {
    let cwd = harness.paths.root.to_string_lossy();
    let fields = format!(
        "script: /bin/sh\ncwd: \"{cwd}\"\nautorestart: {autorestart}\nargs:\n  - \"-c\"\n  - \"{script}\"\n"
    );
    write_both(harness, name, &fields)
}

fn write_both(harness: &Harness, name: &str, fields: &str) -> PathBuf {
    let service = format!("name: {name}\n{fields}");
    std::fs::write(service_file_of(&harness.cfg_dir, name), &service)
        .expect("write the service file");
    let listed = fields.lines().fold(String::new(), |mut text, line| {
        let _ = writeln!(text, "    {line}");
        text
    });
    write_apps_file(
        harness.dir.path(),
        &format!("apps:\n  - name: {name}\n{listed}"),
    )
}

pub async fn next_event(events: &mut mpsc::Receiver<DaemonEvent>) -> DaemonEvent {
    tokio::time::timeout(EVENT_BUDGET, events.recv())
        .await
        .expect("an event should arrive")
        .expect("the event queue should stay open")
}

pub fn command(request: DaemonRequest) -> (DaemonCommand, oneshot::Receiver<DaemonOutcome>) {
    let (reply, answer) = oneshot::channel();
    (DaemonCommand { request, reply }, answer)
}

pub fn selector(name: &str) -> AppSelector {
    AppSelector::Name(name.to_string())
}
