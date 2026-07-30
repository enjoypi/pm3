use std::path::{Path, PathBuf};

use adapters::{Pm3Config, RestartConfig, SandboxConfig, ServiceConfig};

pub const KILL_TIMEOUT_MS: u64 = 400;
pub const START_TIMEOUT_MS: u64 = 4000;
pub const POLL_INTERVAL_MS: u64 = 20;
pub const DRAIN_TIMEOUT_SECS: u64 = 5;
pub const MIN_UPTIME_MS: u64 = 1000;
pub const MAX_RESTARTS: u32 = 15;
pub const SANDBOX_MODE: &str = "danger-full-access";
pub const SERVICE_LABEL: &str = "pm3-fixture";
pub const SERVICE_SEARCH_PATH: &str = "/usr/bin:/bin";

pub fn pm3_config_with_home(home: &str) -> Pm3Config {
    Pm3Config {
        home: home.to_string(),
        kill_timeout_ms: KILL_TIMEOUT_MS,
        start_timeout_ms: START_TIMEOUT_MS,
        drain_timeout_secs: DRAIN_TIMEOUT_SECS,
        daemon_poll_interval_ms: POLL_INTERVAL_MS,
        restart: RestartConfig {
            min_uptime_ms: MIN_UPTIME_MS,
            max_restarts: MAX_RESTARTS,
            restart_delay_ms: 0,
        },
        sandbox: SandboxConfig {
            mode: SANDBOX_MODE.to_string(),
            network: false,
        },
        service: ServiceConfig {
            label: SERVICE_LABEL.to_string(),
            search_path: SERVICE_SEARCH_PATH.to_string(),
        },
    }
}

pub fn config_yaml(home: &str) -> String {
    format!(
        r#"pm3:
  home: "{home}"
  kill_timeout_ms: {KILL_TIMEOUT_MS}
  start_timeout_ms: {START_TIMEOUT_MS}
  drain_timeout_secs: {DRAIN_TIMEOUT_SECS}
  daemon_poll_interval_ms: {POLL_INTERVAL_MS}
  restart:
    min_uptime_ms: {MIN_UPTIME_MS}
    max_restarts: {MAX_RESTARTS}
    restart_delay_ms: 0
  sandbox:
    mode: "{SANDBOX_MODE}"
    network: false
  service:
    label: "{SERVICE_LABEL}"
    search_path: "{SERVICE_SEARCH_PATH}"

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
