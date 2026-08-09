use std::path::{Path, PathBuf};

use adapters::{Pm3Config, RestartConfig, SandboxConfig, ServiceConfig};

pub const STOP_SIGNAL: &str = "TERM";
pub const KILL_TIMEOUT_MS: u64 = 400;
pub const START_TIMEOUT_MS: u64 = 4000;
pub const REQUEST_TIMEOUT_MS: u64 = 30000;
pub const COMMAND_TIMEOUT_MS: u64 = 5000;
pub const POLL_INTERVAL_MS: u64 = 20;
pub const MEMORY_POLL_INTERVAL_MS: u64 = 40;
pub const FOLLOW_INTERVAL_MS: u64 = 200;
pub const LOG_TAIL_LINES: u64 = 20;
pub const DRAIN_TIMEOUT_SECS: u64 = 5;
pub const MIN_UPTIME_MS: u64 = 1000;
pub const MAX_RESTARTS: u32 = 15;
pub const SANDBOX_MODE: &str = "danger-full-access";
pub const SANDBOX_READ: &str = "minimal";
pub const MINIMAL_READ_ROOT: &str = "/usr";
pub const FORBIDDEN_WRITABLE_ROOT: &str = "/";
pub const SERVICE_LABEL: &str = "pm3-fixture";
pub const SERVICE_SEARCH_PATH: &str = "/usr/bin:/bin";
pub const SERVICE_RESTART_DELAY_SECS: u64 = 2;
pub const SERVICE_RESTART_CONDITION: &str = "always";
pub const SERVICE_MAX_TASKS: u64 = 4096;
pub const SERVICE_CPU_QUOTA_PERCENT: u64 = 0;
pub const CHANNEL_DEPTH: usize = 32;
pub const BODY_LIMIT_BYTES: usize = 131_072;
pub const SEATBELT_PROGRAM: &str = "/usr/bin/sandbox-exec";
pub const BWRAP_PROGRAM: &str = "bwrap";
pub const LAUNCHCTL_PATH: &str = "/bin/launchctl";
pub const SYSTEMCTL_PATH: &str = "/usr/bin/systemctl";
pub const LOGINCTL_PATH: &str = "/usr/bin/loginctl";
pub const SCHTASKS_PATH: &str = "schtasks";

pub fn pm3_config_with_home(home: &str) -> Pm3Config {
    Pm3Config {
        home: home.to_string(),
        cfg_dir: format!("{home}/service"),
        search_path: SERVICE_SEARCH_PATH.to_string(),
        stop_signal: STOP_SIGNAL.to_string(),
        kill_timeout_ms: KILL_TIMEOUT_MS,
        start_timeout_ms: START_TIMEOUT_MS,
        drain_timeout_secs: DRAIN_TIMEOUT_SECS,
        request_timeout_ms: REQUEST_TIMEOUT_MS,
        command_timeout_ms: COMMAND_TIMEOUT_MS,
        daemon_poll_interval_ms: POLL_INTERVAL_MS,
        daemon_poll_max_interval_ms: POLL_INTERVAL_MS,
        memory_poll_interval_ms: MEMORY_POLL_INTERVAL_MS,
        log_follow_interval_ms: FOLLOW_INTERVAL_MS,
        log_tail_lines: LOG_TAIL_LINES,
        log_rotate_max_bytes: 0,
        log_rotate_interval_ms: 60000,
        ready_timeout_ms: 30000,
        ready_poll_interval_ms: 200,
        daemon_channel_depth: CHANNEL_DEPTH,
        request_body_limit_bytes: BODY_LIMIT_BYTES,
        restart: RestartConfig {
            autorestart: true,
            min_uptime_ms: MIN_UPTIME_MS,
            max_restarts: MAX_RESTARTS,
            restart_delay_ms: 0,
            max_restart_delay_ms: 15000,
        },
        sandbox: SandboxConfig {
            mode: SANDBOX_MODE.to_string(),
            read: SANDBOX_READ.to_string(),
            network: false,
            seatbelt_program: SEATBELT_PROGRAM.to_string(),
            bwrap_program: BWRAP_PROGRAM.to_string(),
            minimal_read_roots: vec![MINIMAL_READ_ROOT.to_string()],
            forbidden_writable_roots: vec![FORBIDDEN_WRITABLE_ROOT.to_string()],
        },
        service: ServiceConfig {
            label: SERVICE_LABEL.to_string(),
            restart_delay_secs: SERVICE_RESTART_DELAY_SECS,
            restart_condition: SERVICE_RESTART_CONDITION.to_string(),
            max_tasks: SERVICE_MAX_TASKS,
            cpu_quota_percent: SERVICE_CPU_QUOTA_PERCENT,
            wait_for_network: false,
            launchctl_path: LAUNCHCTL_PATH.to_string(),
            systemctl_path: SYSTEMCTL_PATH.to_string(),
            loginctl_path: LOGINCTL_PATH.to_string(),
            schtasks_path: SCHTASKS_PATH.to_string(),
        },
    }
}

pub fn config_yaml(home: &str) -> String {
    format!(
        r#"pm3:
  home: "{home}"
  cfg_dir: "{home}/service"
  search_path: "{SERVICE_SEARCH_PATH}"
  stop_signal: "{STOP_SIGNAL}"
  kill_timeout_ms: {KILL_TIMEOUT_MS}
  start_timeout_ms: {START_TIMEOUT_MS}
  drain_timeout_secs: {DRAIN_TIMEOUT_SECS}
  request_timeout_ms: {REQUEST_TIMEOUT_MS}
  command_timeout_ms: {COMMAND_TIMEOUT_MS}
  daemon_poll_interval_ms: {POLL_INTERVAL_MS}
  daemon_poll_max_interval_ms: {POLL_INTERVAL_MS}
  memory_poll_interval_ms: {MEMORY_POLL_INTERVAL_MS}
  log_follow_interval_ms: {FOLLOW_INTERVAL_MS}
  log_tail_lines: {LOG_TAIL_LINES}
  log_rotate_max_bytes: 0
  log_rotate_interval_ms: 60000
  ready_timeout_ms: 30000
  ready_poll_interval_ms: 200
  daemon_channel_depth: {CHANNEL_DEPTH}
  request_body_limit_bytes: {BODY_LIMIT_BYTES}
  restart:
    autorestart: true
    min_uptime_ms: {MIN_UPTIME_MS}
    max_restarts: {MAX_RESTARTS}
    restart_delay_ms: 0
    max_restart_delay_ms: 15000
  sandbox:
    mode: "{SANDBOX_MODE}"
    read: "{SANDBOX_READ}"
    network: false
    seatbelt_program: "{SEATBELT_PROGRAM}"
    bwrap_program: "{BWRAP_PROGRAM}"
    minimal_read_roots:
      - "{MINIMAL_READ_ROOT}"
    forbidden_writable_roots:
      - "{FORBIDDEN_WRITABLE_ROOT}"
  service:
    label: "{SERVICE_LABEL}"
    restart_delay_secs: {SERVICE_RESTART_DELAY_SECS}
    restart_condition: "{SERVICE_RESTART_CONDITION}"
    max_tasks: {SERVICE_MAX_TASKS}
    cpu_quota_percent: {SERVICE_CPU_QUOTA_PERCENT}
    wait_for_network: false
    launchctl_path: "{LAUNCHCTL_PATH}"
    systemctl_path: "{SYSTEMCTL_PATH}"
    loginctl_path: "{LOGINCTL_PATH}"
    schtasks_path: "{SCHTASKS_PATH}"

telemetry:
  service_name: "pm3"
  log_level: "info"
  log_format: "json"
"#
    )
}

pub fn write_config(dir: &Path, home: &str) -> PathBuf {
    let path = dir.join("config.yaml");
    std::fs::write(&path, config_yaml(home)).expect("write the pm3 config");
    path
}

#[cfg(unix)]
pub fn write_config_with_cfg_dir(dir: &Path, home: &str, cfg_dir: &str) -> PathBuf {
    let path = dir.join("config.yaml");
    let yaml = config_yaml(home).replace(
        &format!("cfg_dir: \"{home}/service\""),
        &format!("cfg_dir: \"{cfg_dir}\""),
    );
    std::fs::write(&path, yaml).expect("write the pm3 config");
    path
}

#[cfg(unix)]
pub fn write_impatient_config(dir: &Path, home: &str) -> PathBuf {
    let path = dir.join("config.yaml");
    let yaml = config_yaml(home).replace(
        &format!("start_timeout_ms: {START_TIMEOUT_MS}"),
        "start_timeout_ms: 60",
    );
    std::fs::write(&path, yaml).expect("write the pm3 config");
    path
}

pub fn write_apps_file(dir: &Path, body: &str) -> PathBuf {
    let path = dir.join("apps.yaml");
    std::fs::write(&path, body).expect("write the apps file");
    path
}
