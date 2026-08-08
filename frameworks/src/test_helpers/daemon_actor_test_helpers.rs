use std::{fmt::Write as _, path::PathBuf, time::Duration};

use adapters::{
    AppSelector, DaemonCommand, Pm3Paths, SupervisionOutcome, SupervisionRequest, resolve_paths,
    service_file_of,
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
    pub ports: Arc<DaemonPorts>,
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

pub fn harness_with_log_rotate(max_bytes: u64, interval_ms: u64) -> Harness {
    built_harness_with_rotate(
        pm3_config_with_home("/unused").kill_timeout_ms,
        SANDBOX_MODE,
        max_bytes,
        interval_ms,
    )
}

fn built_harness(kill_timeout_ms: u64, sandbox_mode: &str) -> Harness {
    built_harness_with_rotate(kill_timeout_ms, sandbox_mode, 0, 60000)
}

fn built_harness_with_rotate(
    kill_timeout_ms: u64,
    sandbox_mode: &str,
    log_rotate_max_bytes: u64,
    log_rotate_interval_ms: u64,
) -> Harness {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = resolve_paths(dir.path());
    std::fs::create_dir_all(&paths.logs_dir).expect("create the log directory");
    let cfg_dir = dir.path().join("service");
    std::fs::create_dir_all(&cfg_dir).expect("create the service directory");
    let mut config = pm3_config_with_home(&paths.root.to_string_lossy());
    config.kill_timeout_ms = kill_timeout_ms;
    config.sandbox.mode = sandbox_mode.to_string();
    config.log_rotate_max_bytes = log_rotate_max_bytes;
    config.log_rotate_interval_ms = log_rotate_interval_ms;
    let specs = SpecSource {
        cfg_dir: cfg_dir.clone(),
        config,
        home_dir: paths.root.to_string_lossy().into_owned(),
        host_home: None,
        logs_dir: paths.logs_dir.to_string_lossy().into_owned(),
        tmp_dir: None,
    };
    let ports = Arc::new(DaemonPorts::new(
        paths.dump_file.clone(),
        specs.clone(),
        None,
    ));
    let (sender, events) = mpsc::channel(CHANNEL_DEPTH);
    let daemon = Daemon::new(specs, Arc::clone(&ports), sender.clone());
    Harness {
        dir,
        paths,
        cfg_dir,
        daemon,
        ports,
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
    let cwd = workspace_of(harness);
    let fields = format!(
        "script: /bin/sh\ncwd: \"{cwd}\"\nautorestart: false\nschedule: \"{cron}\"\nargs:\n  - \"-c\"\n  - \"{script}\"\n"
    );
    write_both(harness, name, &fields)
}

pub fn scheduled_online_apps_file(
    harness: &Harness,
    name: &str,
    script: &str,
    cron: &str,
) -> PathBuf {
    let cwd = workspace_of(harness);
    let fields = format!(
        "script: /bin/sh\ncwd: \"{cwd}\"\nautorestart: true\nschedule: \"{cron}\"\nargs:\n  - \"-c\"\n  - \"{script}\"\n"
    );
    write_both(harness, name, &fields)
}

fn written_apps_file(harness: &Harness, name: &str, script: &str, autorestart: bool) -> PathBuf {
    let cwd = workspace_of(harness);
    let fields = format!(
        "script: /bin/sh\ncwd: \"{cwd}\"\nautorestart: {autorestart}\nargs:\n  - \"-c\"\n  - \"{script}\"\n"
    );
    write_both(harness, name, &fields)
}

fn write_both(harness: &Harness, name: &str, fields: &str) -> PathBuf {
    let service = format!("name: {name}\n{fields}");
    std::fs::write(
        service_file_of(&harness.cfg_dir, name).expect("a safe service name"),
        &service,
    )
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

pub fn command(
    request: SupervisionRequest,
) -> (DaemonCommand, oneshot::Receiver<SupervisionOutcome>) {
    let (reply, answer) = oneshot::channel();
    (DaemonCommand { request, reply }, answer)
}

pub fn queue_restart(harness: &mut Harness, name: &str, delay_ms: u64) {
    let effect = harness.daemon.supervisor.queue_restart(name, delay_ms);
    harness.daemon.board.apply(effect);
}

pub fn selector(name: &str) -> AppSelector {
    AppSelector::Name(name.to_string())
}

pub fn capped_apps_file(harness: &Harness, name: &str, script: &str, max_memory: &str) -> PathBuf {
    let cwd = workspace_of(harness);
    let fields = format!(
        "script: /bin/sh\ncwd: \"{cwd}\"\nautorestart: true\nmax_memory: \"{max_memory}\"\nargs:\n  - \"-c\"\n  - \"{script}\"\n"
    );
    write_both(harness, name, &fields)
}

pub fn clean_exit_apps_file(harness: &Harness, name: &str, code: i32) -> PathBuf {
    let cwd = workspace_of(harness);
    let fields = format!(
        "script: /bin/sh\ncwd: \"{cwd}\"\nautorestart: true\nmin_uptime_ms: 50\nmax_restarts: 1\nstop_exit_codes:\n  - {code}\nargs:\n  - \"-c\"\n  - \"exit {code}\"\n"
    );
    write_both(harness, name, &fields)
}

pub fn workspace_of(harness: &Harness) -> String {
    let workspace = harness.paths.root.join("work");
    std::fs::create_dir_all(&workspace).expect("prepare the workspace");
    workspace.to_string_lossy().into_owned()
}
