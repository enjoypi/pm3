#![allow(
    dead_code,
    reason = "each integration test binary consumes a different subset of these helpers"
)]

use std::{
    io::Write as _,
    path::{Path, PathBuf},
    process::Output,
    time::Duration,
};

pub const PM3: &str = env!("CARGO_BIN_EXE_pm3");
pub const SERVICE_LABEL: &str = "pm3-e2e-never-installed";
pub const READY_BUDGET: Duration = Duration::from_secs(15);
pub const PROBE_INTERVAL: Duration = Duration::from_millis(50);
pub const START_TIMEOUT_MS: u64 = 8000;

pub struct Home {
    pub dir: tempfile::TempDir,
    pub root: PathBuf,
    pub config: PathBuf,
}

impl Drop for Home {
    fn drop(&mut self) {
        if !self.root.join("pm3.pid").exists() {
            return;
        }
        for args in [["list"].as_slice(), ["kill", "--with-services"].as_slice()] {
            std::process::Command::new(PM3)
                .arg("--config")
                .arg(&self.config)
                .args(args)
                .output()
                .ok();
        }
    }
}

pub const FULL_READ: &str = "full";
pub const MINIMAL_READ: &str = "minimal";

pub fn home_with_sandbox(mode: &str, network: bool) -> Home {
    home_with(mode, network, "info")
}

pub fn home_with_read_scope(mode: &str, network: bool, read: &str) -> Home {
    build_home(mode, read, network, "info", START_TIMEOUT_MS)
}

pub fn home() -> Home {
    home_with("danger-full-access", true, "info")
}

pub fn verbose_home() -> Home {
    home_with("danger-full-access", true, "debug")
}

pub fn impatient_home() -> Home {
    home_with_timeout("danger-full-access", true, "info", 200)
}

pub fn home_with(mode: &str, network: bool, log_level: &str) -> Home {
    home_with_timeout(mode, network, log_level, START_TIMEOUT_MS)
}

pub fn home_with_timeout(
    mode: &str,
    network: bool,
    log_level: &str,
    start_timeout_ms: u64,
) -> Home {
    build_home(mode, FULL_READ, network, log_level, start_timeout_ms)
}

#[derive(Copy, Clone)]
pub struct HomeTunables {
    pub memory_poll_interval_ms: u64,
    pub log_rotate_max_bytes: u64,
    pub log_rotate_interval_ms: u64,
}

impl Default for HomeTunables {
    fn default() -> Self {
        Self {
            memory_poll_interval_ms: 30000,
            log_rotate_max_bytes: 0,
            log_rotate_interval_ms: 60000,
        }
    }
}

pub fn home_with_memory_poll(memory_poll_interval_ms: u64) -> Home {
    build_home_full(
        "danger-full-access",
        FULL_READ,
        true,
        "info",
        START_TIMEOUT_MS,
        HomeTunables {
            memory_poll_interval_ms,
            ..HomeTunables::default()
        },
    )
}

pub fn home_with_log_rotate(max_bytes: u64, interval_ms: u64) -> Home {
    build_home_full(
        "danger-full-access",
        FULL_READ,
        true,
        "debug",
        START_TIMEOUT_MS,
        HomeTunables {
            log_rotate_max_bytes: max_bytes,
            log_rotate_interval_ms: interval_ms,
            ..HomeTunables::default()
        },
    )
}

fn build_home(
    mode: &str,
    read: &str,
    network: bool,
    log_level: &str,
    start_timeout_ms: u64,
) -> Home {
    build_home_full(
        mode,
        read,
        network,
        log_level,
        start_timeout_ms,
        HomeTunables::default(),
    )
}

fn build_home_full(
    mode: &str,
    read: &str,
    network: bool,
    log_level: &str,
    start_timeout_ms: u64,
    tunables: HomeTunables,
) -> Home {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path().join("home");
    std::fs::create_dir_all(root.join("logs")).expect("prepare the pm3 home");
    let config = dir.path().join("config.yaml");
    std::fs::write(
        &config,
        config_yaml(
            &root.to_string_lossy(),
            mode,
            read,
            network,
            log_level,
            start_timeout_ms,
            &tunables,
        ),
    )
    .expect("write the pm3 config");
    Home { dir, root, config }
}

pub fn config_yaml(
    home: &str,
    sandbox_mode: &str,
    sandbox_read: &str,
    network: bool,
    log_level: &str,
    start_timeout_ms: u64,
    tunables: &HomeTunables,
) -> String {
    let memory_poll_interval_ms = tunables.memory_poll_interval_ms;
    let log_rotate_max_bytes = tunables.log_rotate_max_bytes;
    let log_rotate_interval_ms = tunables.log_rotate_interval_ms;
    format!(
        r#"pm3:
  home: "{home}"
  cfg_dir: "{home}/service"
  search_path: "/usr/bin:/bin:/opt/homebrew/bin"
  stop_signal: "TERM"
  kill_timeout_ms: 400
  start_timeout_ms: {start_timeout_ms}
  drain_timeout_secs: 2
  request_timeout_ms: 30000
  command_timeout_ms: 5000
  daemon_poll_interval_ms: 40
  daemon_poll_max_interval_ms: 200
  memory_poll_interval_ms: {memory_poll_interval_ms}
  log_follow_interval_ms: 200
  log_tail_lines: 20
  log_rotate_max_bytes: {log_rotate_max_bytes}
  log_rotate_interval_ms: {log_rotate_interval_ms}
  ready_timeout_ms: 30000
  ready_poll_interval_ms: 200
  daemon_channel_depth: 32
  request_body_limit_bytes: 131072
  restart:
    autorestart: true
    min_uptime_ms: 1000
    max_restarts: 15
    restart_delay_ms: 0
    max_restart_delay_ms: 15000
  sandbox:
    mode: "{sandbox_mode}"
    read: "{sandbox_read}"
    network: {network}
    seatbelt_program: "/usr/bin/sandbox-exec"
    bwrap_program: "bwrap"
    minimal_read_roots:
      - "/bin"
      - "/sbin"
      - "/usr"
      - "/etc"
      - "/lib"
      - "/lib64"
      - "/opt/homebrew"
    forbidden_writable_roots:
      - "/"
      - "/etc"
      - "/usr"
  service:
    label: "{SERVICE_LABEL}"
    restart_delay_secs: 2
    restart_condition: "always"
    max_tasks: 4096
    cpu_quota_percent: 0
    launchctl_path: "/bin/launchctl"
    systemctl_path: "/usr/bin/systemctl"
    loginctl_path: "/usr/bin/loginctl"

telemetry:
  service_name: "pm3"
  log_level: "{log_level}"
  log_format: "json"
"#
    )
}

pub fn netcat() -> &'static str {
    ["/usr/bin/nc", "/bin/nc"]
        .into_iter()
        .find(|candidate| Path::new(candidate).exists())
        .expect("the host should provide nc for the network probe")
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

pub fn pm3_with_stdin(home: &Home, args: &[&str], input: &str) -> Output {
    let mut child = std::process::Command::new(PM3)
        .arg("--config")
        .arg(&home.config)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("pm3 should run");
    {
        let mut stdin = child.stdin.take().expect("stdin is piped");
        stdin.write_all(input.as_bytes()).expect("write the answer");
    }
    child.wait_with_output().expect("pm3 should exit")
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

pub fn wait_for_listing(home: &Home, needle: &str) -> String {
    let deadline = std::time::Instant::now() + READY_BUDGET;
    let mut shown = String::new();
    while std::time::Instant::now() < deadline {
        shown = stdout_of(&pm3(home, &["list"]));
        if shown.contains(needle) {
            return shown;
        }
        std::thread::sleep(PROBE_INTERVAL);
    }
    panic!("the listing should mention {needle} inside the budget, saw:\n{shown}")
}

pub fn signal(pid: u32, name: &str) {
    let status = std::process::Command::new("/bin/kill")
        .args([name, &pid.to_string()])
        .status()
        .expect("should signal the daemon");
    assert!(status.success(), "kill {name} {pid} should succeed");
}

pub fn shutdown_daemon(home: &Home) {
    let listed = pm3(home, &["list"]);
    assert!(listed.status.success(), "{}", stdout_of(&listed));
    let killed = pm3(home, &["kill", "--with-services"]);
    assert!(killed.status.success(), "{}", stdout_of(&killed));
    wait_until_gone(&home.root.join("pm3.sock"));
}

pub fn detach_daemon(home: &Home) {
    let socket = home.root.join("pm3.sock");
    signal(daemon_pid(home), "-TERM");
    wait_until_gone(&socket);
}

pub fn process_is_alive(pid: u32) -> bool {
    std::process::Command::new("/bin/kill")
        .args(["-0", &pid.to_string()])
        .output()
        .expect("should probe the process")
        .status
        .success()
}

pub fn described_pid(home: &Home, name: &str) -> u32 {
    let described = pm3(home, &["describe", name]);
    let text = stdout_of(&described);
    let line = text
        .lines()
        .find(|line| line.trim_start().starts_with("pid"))
        .unwrap_or_else(|| panic!("describe should report a pid, got: {text}"));
    line.rsplit_once(' ')
        .map(|(_label, pid)| pid.trim())
        .and_then(|pid| pid.parse().ok())
        .unwrap_or_else(|| panic!("describe should report a numeric pid, got: {line}"))
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

pub fn workspace_of(home: &Home) -> String {
    let workspace = home.root.join("work");
    std::fs::create_dir_all(&workspace).expect("prepare the workspace");
    workspace.to_string_lossy().into_owned()
}
