#![allow(
    dead_code,
    reason = "each integration test binary consumes a different subset of these helpers"
)]

use std::{
    path::{Path, PathBuf},
    process::Output,
    time::Duration,
};

pub const PM3: &str = env!("CARGO_BIN_EXE_pm3");
pub const SERVICE_LABEL: &str = "pm3-e2e-never-installed";
pub const READY_BUDGET: Duration = Duration::from_secs(15);
pub const PROBE_INTERVAL: Duration = Duration::from_millis(50);

pub struct Home {
    pub dir: tempfile::TempDir,
    pub root: PathBuf,
    pub config: PathBuf,
}

pub fn home_with_sandbox(mode: &str, network: bool) -> Home {
    home_with(mode, network, "info")
}

pub fn home() -> Home {
    home_with("danger-full-access", true, "info")
}

pub fn verbose_home() -> Home {
    home_with("danger-full-access", true, "debug")
}

pub fn home_with(mode: &str, network: bool, log_level: &str) -> Home {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path().join("home");
    std::fs::create_dir_all(root.join("logs")).expect("prepare the pm3 home");
    let config = dir.path().join("config.yaml");
    std::fs::write(
        &config,
        config_yaml(&root.to_string_lossy(), mode, network, log_level),
    )
    .expect("write the pm3 config");
    Home { dir, root, config }
}

pub fn config_yaml(home: &str, sandbox_mode: &str, network: bool, log_level: &str) -> String {
    format!(
        r#"pm3:
  home: "{home}"
  kill_timeout_ms: 400
  start_timeout_ms: 8000
  drain_timeout_secs: 2
  daemon_poll_interval_ms: 40
  restart:
    min_uptime_ms: 1000
    max_restarts: 15
    restart_delay_ms: 0
  sandbox:
    mode: "{sandbox_mode}"
    network: {network}
  service:
    label: "{SERVICE_LABEL}"
    search_path: "/usr/bin:/bin"

telemetry:
  service_name: "pm3"
  log_level: "{log_level}"
  log_format: "json"
"#
    )
}

pub fn write_apps(home: &Home, body: &str) -> PathBuf {
    let path = home.dir.path().join("apps.yaml");
    std::fs::write(&path, body).expect("write the apps file");
    path
}

pub fn sleeper_apps(home: &Home, name: &str) -> PathBuf {
    let cwd = home.root.to_string_lossy();
    write_apps(
        home,
        &format!(
            "apps:\n  - name: {name}\n    script: {PM3}\n    cwd: \"{cwd}\"\n    args:\n      - \"__sleep\"\n      - \"30000\"\n"
        ),
    )
}

pub fn pm3(home: &Home, args: &[&str]) -> Output {
    let mut command = std::process::Command::new(PM3);
    command
        .arg("--config")
        .arg(&home.config)
        .args(args)
        .output()
        .expect("pm3 should run")
}

pub fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

pub fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

pub fn daemon_pid(home: &Home) -> u32 {
    let raw = std::fs::read_to_string(home.root.join("pm3.pid")).expect("the daemon pid file");
    raw.trim().parse().expect("a numeric pid")
}

pub fn wait_for_file(path: &Path) {
    let deadline = std::time::Instant::now() + READY_BUDGET;
    while std::time::Instant::now() < deadline {
        if path.exists() {
            return;
        }
        std::thread::sleep(PROBE_INTERVAL);
    }
    panic!("{} should appear inside the budget", path.display())
}

pub fn wait_until_gone(path: &Path) {
    let deadline = std::time::Instant::now() + READY_BUDGET;
    while std::time::Instant::now() < deadline {
        if !path.exists() {
            return;
        }
        std::thread::sleep(PROBE_INTERVAL);
    }
    panic!("{} should disappear inside the budget", path.display())
}

pub fn wait_for_log(path: &Path, needle: &str) -> String {
    let deadline = std::time::Instant::now() + READY_BUDGET;
    while std::time::Instant::now() < deadline {
        let seen = std::fs::read_to_string(path).unwrap_or_default();
        if seen.contains(needle) {
            return seen;
        }
        std::thread::sleep(PROBE_INTERVAL);
    }
    panic!("{} should mention {needle}", path.display())
}

pub fn signal(pid: u32, name: &str) {
    let status = std::process::Command::new("/bin/kill")
        .args([name, &pid.to_string()])
        .status()
        .expect("should signal the daemon");
    assert!(status.success(), "kill {name} {pid} should succeed");
}

pub fn shutdown_daemon(home: &Home) {
    let socket = home.root.join("pm3.sock");
    if !socket.exists() {
        return;
    }
    signal(daemon_pid(home), "-TERM");
    wait_until_gone(&socket);
}

pub fn app_log(home: &Home, name: &str) -> PathBuf {
    home.root.join("logs").join(format!("{name}-out.log"))
}

pub fn app_error_log(home: &Home, name: &str) -> PathBuf {
    home.root.join("logs").join(format!("{name}-err.log"))
}

pub fn daemon_log(home: &Home) -> PathBuf {
    home.root.join("pm3.log")
}
