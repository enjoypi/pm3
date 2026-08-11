use serde::{Deserialize, Serialize};
use thiserror::Error;
use usecases::{ReadScope, SandboxMode, is_name_letter};

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("cannot parse config: {0}")]
    ParseError(String),

    #[error("cannot accept empty pm3.home")]
    InvalidHome,

    #[error("cannot accept empty pm3.cfg_dir")]
    InvalidCfgDir,

    #[error("cannot accept pm3.kill_timeout_ms {0}: must be >= 1")]
    InvalidKillTimeout(u64),

    #[error("cannot accept pm3.start_timeout_ms {0}: must be >= 1")]
    InvalidStartTimeout(u64),

    #[error("cannot accept pm3.drain_timeout_secs {0}: must be >= 1")]
    InvalidDrainTimeout(u64),

    #[error("cannot accept pm3.request_timeout_ms {0}: must be >= 1")]
    InvalidRequestTimeout(u64),

    #[error("cannot accept pm3.command_timeout_ms {0}: must be >= 1")]
    InvalidCommandTimeout(u64),

    #[error("cannot accept pm3.daemon_poll_interval_ms {0}: must be >= 1")]
    InvalidPollInterval(u64),

    #[error(
        "cannot accept pm3.daemon_poll_max_interval_ms {max}: must be >= daemon_poll_interval_ms {floor}"
    )]
    InvalidPollCeiling { max: u64, floor: u64 },

    #[error("cannot accept pm3.log_follow_interval_ms {0}: must be >= 1")]
    InvalidFollowInterval(u64),

    #[error("cannot accept pm3.log_tail_lines {0}: must be >= 1")]
    InvalidLogTailLines(u64),

    #[error("cannot accept pm3.log_read_max_bytes {0}: must be >= 1")]
    InvalidLogReadMaxBytes(u64),

    #[error("cannot accept pm3.log_rotate_interval_ms {0}: must be >= 1")]
    InvalidLogRotateInterval(u64),

    #[error("cannot accept pm3.ready_timeout_ms {0}: must be >= 1")]
    InvalidReadyTimeout(u64),

    #[error("cannot accept pm3.ready_poll_interval_ms {0}: must be >= 1")]
    InvalidReadyPollInterval(u64),

    #[error("cannot accept pm3.daemon_channel_depth {0}: must be >= 1")]
    InvalidChannelDepth(usize),

    #[error("cannot accept pm3.request_body_limit_bytes {0}: must be >= 1")]
    InvalidBodyLimit(usize),

    #[error("cannot accept pm3.service.max_tasks {0}: must be >= 1")]
    InvalidMaxTasks(u64),

    #[error("cannot accept empty {field}")]
    EmptyProgram { field: &'static str },

    #[error("cannot accept pm3.service.restart_condition {0}: must be one of always, on-failure")]
    InvalidRestartCondition(String),

    #[error("cannot accept pm3.stop_signal {0}: must be one of TERM, INT, QUIT, HUP, USR1, USR2")]
    InvalidStopSignal(String),

    #[error("cannot accept pm3.restart.min_uptime_ms {0}: must be >= 1")]
    InvalidMinUptime(u64),

    #[error("cannot accept pm3.restart.max_restart_delay_ms {0}: must be >= 1")]
    InvalidMaxRestartDelay(u64),

    #[error("cannot accept pm3.memory_poll_interval_ms {0}: must be >= 1")]
    InvalidMemoryPollInterval(u64),

    #[error("cannot accept pm3.sandbox.mode {mode}: must be one of {expected}")]
    InvalidSandboxMode { mode: String, expected: String },

    #[error("cannot accept pm3.sandbox.read {read}: must be one of {expected}")]
    InvalidSandboxRead { read: String, expected: String },

    #[error("cannot accept {field} entry '{root}': must be an absolute path")]
    RelativeSandboxRoot { field: &'static str, root: String },

    #[error(
        "cannot accept empty pm3.sandbox.minimal_read_roots: a confined read scope needs at least the system directories"
    )]
    EmptyMinimalReadRoots,

    #[error("cannot accept empty pm3.service.label")]
    InvalidServiceLabel,

    #[error("cannot accept pm3.service.label {label}: unsafe character '{character}'")]
    UnsafeServiceLabel { label: String, character: char },

    #[error("cannot accept pm3.service.label {0}: must not start with '.'")]
    DottedServiceLabel(String),

    #[error("cannot accept {field}: must not contain a line break")]
    UnsafeLineBreak { field: &'static str },

    #[error("cannot accept empty pm3.search_path")]
    InvalidSearchPath,

    #[error("cannot accept empty telemetry.service_name")]
    InvalidServiceName,

    #[error(
        "cannot accept telemetry.log_level {0}: must be one of trace, debug, info, warn, error"
    )]
    InvalidLogLevel(String),

    #[error("cannot accept telemetry.log_format {0}: must be one of json, pretty")]
    InvalidLogFormat(String),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AppConfig {
    pub pm3: Pm3Config,
    pub telemetry: TelemetryConfig,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Pm3Config {
    pub home: String,
    pub cfg_dir: String,
    pub search_path: String,
    pub stop_signal: String,
    pub kill_timeout_ms: u64,
    pub start_timeout_ms: u64,
    pub drain_timeout_secs: u64,
    pub request_timeout_ms: u64,
    pub command_timeout_ms: u64,
    pub daemon_poll_interval_ms: u64,
    pub daemon_poll_max_interval_ms: u64,
    pub memory_poll_interval_ms: u64,
    pub log_follow_interval_ms: u64,
    pub log_tail_lines: u64,
    #[serde(default = "default_log_read_max_bytes")]
    pub log_read_max_bytes: u64,
    pub log_rotate_max_bytes: u64,
    pub log_rotate_interval_ms: u64,
    pub ready_timeout_ms: u64,
    pub ready_poll_interval_ms: u64,
    pub daemon_channel_depth: usize,
    pub request_body_limit_bytes: usize,
    pub restart: RestartConfig,
    pub sandbox: SandboxConfig,
    pub service: ServiceConfig,
}

#[derive(Copy, Clone, Debug, Deserialize, Serialize)]
pub struct RestartConfig {
    pub autorestart: bool,
    pub min_uptime_ms: u64,
    pub max_restarts: u32,
    pub restart_delay_ms: u64,
    pub max_restart_delay_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SandboxConfig {
    pub mode: String,
    pub read: String,
    pub network: bool,
    pub seatbelt_program: String,
    pub bwrap_program: String,
    pub minimal_read_roots: Vec<String>,
    pub forbidden_writable_roots: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ServiceConfig {
    pub label: String,
    pub restart_delay_secs: u64,
    pub restart_condition: String,
    pub max_tasks: u64,
    pub cpu_quota_percent: u64,
    pub wait_for_network: bool,
    pub launchctl_path: String,
    pub systemctl_path: String,
    pub loginctl_path: String,
    pub schtasks_path: String,
    pub taskkill_path: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TelemetryConfig {
    pub service_name: String,
    pub log_level: String,
    pub log_format: String,
}

pub const DEFAULT_LOG_READ_MAX_BYTES: u64 = 4 * 1024 * 1024;

const fn default_log_read_max_bytes() -> u64 {
    DEFAULT_LOG_READ_MAX_BYTES
}

pub const LOG_FORMAT_JSON: &str = "json";
pub const LOG_FORMAT_PRETTY: &str = "pretty";

pub const STOP_SIGNAL_TERM: &str = "TERM";

pub const RESTART_CONDITION_ALWAYS: &str = "always";
pub const RESTART_CONDITION_ON_FAILURE: &str = "on-failure";

const VALID_LOG_LEVELS: &[&str] = &["trace", "debug", "info", "warn", "error"];
const VALID_LOG_FORMATS: &[&str] = &[LOG_FORMAT_JSON, LOG_FORMAT_PRETTY];
const VALID_RESTART_CONDITIONS: &[&str] = &[RESTART_CONDITION_ALWAYS, RESTART_CONDITION_ON_FAILURE];

pub fn validate_config(cfg: &AppConfig) -> Result<(), ConfigError> {
    validate_pm3_config(&cfg.pm3)?;
    validate_telemetry_config(&cfg.telemetry)
}

pub fn validate_pm3_config(pm3: &Pm3Config) -> Result<(), ConfigError> {
    validate_paths(pm3)?;
    validate_budgets(pm3)?;
    validate_choices(pm3)?;
    validate_service_label(&pm3.service.label)?;
    validate_programs(pm3)
}

fn validate_paths(pm3: &Pm3Config) -> Result<(), ConfigError> {
    if pm3.home.is_empty() {
        return Err(ConfigError::InvalidHome);
    }
    if pm3.cfg_dir.is_empty() {
        return Err(ConfigError::InvalidCfgDir);
    }
    if pm3.search_path.is_empty() {
        return Err(ConfigError::InvalidSearchPath);
    }
    reject_line_break("pm3.home", &pm3.home)?;
    reject_line_break("pm3.search_path", &pm3.search_path)
}

const fn validate_budgets(pm3: &Pm3Config) -> Result<(), ConfigError> {
    if pm3.kill_timeout_ms < 1 {
        return Err(ConfigError::InvalidKillTimeout(pm3.kill_timeout_ms));
    }
    if pm3.start_timeout_ms < 1 {
        return Err(ConfigError::InvalidStartTimeout(pm3.start_timeout_ms));
    }
    if pm3.drain_timeout_secs < 1 {
        return Err(ConfigError::InvalidDrainTimeout(pm3.drain_timeout_secs));
    }
    if pm3.request_timeout_ms < 1 {
        return Err(ConfigError::InvalidRequestTimeout(pm3.request_timeout_ms));
    }
    if pm3.command_timeout_ms < 1 {
        return Err(ConfigError::InvalidCommandTimeout(pm3.command_timeout_ms));
    }
    if pm3.daemon_poll_interval_ms < 1 {
        return Err(ConfigError::InvalidPollInterval(
            pm3.daemon_poll_interval_ms,
        ));
    }
    if pm3.daemon_poll_max_interval_ms < pm3.daemon_poll_interval_ms {
        return Err(ConfigError::InvalidPollCeiling {
            max: pm3.daemon_poll_max_interval_ms,
            floor: pm3.daemon_poll_interval_ms,
        });
    }
    if pm3.memory_poll_interval_ms < 1 {
        return Err(ConfigError::InvalidMemoryPollInterval(
            pm3.memory_poll_interval_ms,
        ));
    }
    if pm3.log_follow_interval_ms < 1 {
        return Err(ConfigError::InvalidFollowInterval(
            pm3.log_follow_interval_ms,
        ));
    }
    if pm3.log_tail_lines < 1 {
        return Err(ConfigError::InvalidLogTailLines(pm3.log_tail_lines));
    }
    if pm3.log_read_max_bytes < 1 {
        return Err(ConfigError::InvalidLogReadMaxBytes(pm3.log_read_max_bytes));
    }
    if pm3.log_rotate_interval_ms < 1 {
        return Err(ConfigError::InvalidLogRotateInterval(
            pm3.log_rotate_interval_ms,
        ));
    }
    if pm3.ready_timeout_ms < 1 {
        return Err(ConfigError::InvalidReadyTimeout(pm3.ready_timeout_ms));
    }
    if pm3.ready_poll_interval_ms < 1 {
        return Err(ConfigError::InvalidReadyPollInterval(
            pm3.ready_poll_interval_ms,
        ));
    }
    if pm3.daemon_channel_depth < 1 {
        return Err(ConfigError::InvalidChannelDepth(pm3.daemon_channel_depth));
    }
    if pm3.request_body_limit_bytes < 1 {
        return Err(ConfigError::InvalidBodyLimit(pm3.request_body_limit_bytes));
    }
    Ok(())
}

fn validate_choices(pm3: &Pm3Config) -> Result<(), ConfigError> {
    if !usecases::VALID_SIGNALS.contains(&pm3.stop_signal.as_str()) {
        return Err(ConfigError::InvalidStopSignal(pm3.stop_signal.clone()));
    }
    if pm3.restart.min_uptime_ms < 1 {
        return Err(ConfigError::InvalidMinUptime(pm3.restart.min_uptime_ms));
    }
    if pm3.restart.max_restart_delay_ms < 1 {
        return Err(ConfigError::InvalidMaxRestartDelay(
            pm3.restart.max_restart_delay_ms,
        ));
    }
    if SandboxMode::parse(&pm3.sandbox.mode).is_none() {
        return Err(ConfigError::InvalidSandboxMode {
            mode: pm3.sandbox.mode.clone(),
            expected: sandbox_mode_names(),
        });
    }
    if ReadScope::parse(&pm3.sandbox.read).is_none() {
        return Err(ConfigError::InvalidSandboxRead {
            read: pm3.sandbox.read.clone(),
            expected: read_scope_names(),
        });
    }
    validate_sandbox_roots(&pm3.sandbox)
}

fn validate_sandbox_roots(sandbox: &SandboxConfig) -> Result<(), ConfigError> {
    if sandbox.minimal_read_roots.is_empty() {
        return Err(ConfigError::EmptyMinimalReadRoots);
    }
    reject_relative_roots(
        "pm3.sandbox.minimal_read_roots",
        &sandbox.minimal_read_roots,
    )?;
    reject_relative_roots(
        "pm3.sandbox.forbidden_writable_roots",
        &sandbox.forbidden_writable_roots,
    )
}

fn reject_relative_roots(field: &'static str, roots: &[String]) -> Result<(), ConfigError> {
    roots
        .iter()
        .find(|root| !root.starts_with('/'))
        .map_or(Ok(()), |root| {
            Err(ConfigError::RelativeSandboxRoot {
                field,
                root: root.clone(),
            })
        })
}

fn validate_programs(pm3: &Pm3Config) -> Result<(), ConfigError> {
    reject_empty(
        "pm3.sandbox.seatbelt_program",
        &pm3.sandbox.seatbelt_program,
    )?;
    reject_empty("pm3.sandbox.bwrap_program", &pm3.sandbox.bwrap_program)?;
    reject_empty("pm3.service.launchctl_path", &pm3.service.launchctl_path)?;
    reject_empty("pm3.service.systemctl_path", &pm3.service.systemctl_path)?;
    reject_empty("pm3.service.loginctl_path", &pm3.service.loginctl_path)?;
    reject_empty("pm3.service.schtasks_path", &pm3.service.schtasks_path)?;
    reject_empty("pm3.service.taskkill_path", &pm3.service.taskkill_path)?;
    if !VALID_RESTART_CONDITIONS.contains(&pm3.service.restart_condition.as_str()) {
        return Err(ConfigError::InvalidRestartCondition(
            pm3.service.restart_condition.clone(),
        ));
    }
    if pm3.service.max_tasks < 1 {
        return Err(ConfigError::InvalidMaxTasks(pm3.service.max_tasks));
    }
    Ok(())
}

const fn reject_empty(field: &'static str, value: &str) -> Result<(), ConfigError> {
    if value.is_empty() {
        return Err(ConfigError::EmptyProgram { field });
    }
    Ok(())
}

fn validate_service_label(label: &str) -> Result<(), ConfigError> {
    if label.is_empty() {
        return Err(ConfigError::InvalidServiceLabel);
    }
    if label.starts_with('.') {
        return Err(ConfigError::DottedServiceLabel(label.to_string()));
    }
    label
        .chars()
        .find(|letter| !is_name_letter(*letter))
        .map_or(Ok(()), |character| {
            Err(ConfigError::UnsafeServiceLabel {
                label: label.to_string(),
                character,
            })
        })
}

fn reject_line_break(field: &'static str, value: &str) -> Result<(), ConfigError> {
    if value.contains(['\n', '\r']) {
        return Err(ConfigError::UnsafeLineBreak { field });
    }
    Ok(())
}

fn sandbox_mode_names() -> String {
    SandboxMode::ALL
        .iter()
        .map(|mode| mode.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn read_scope_names() -> String {
    ReadScope::ALL
        .iter()
        .map(|scope| scope.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn validate_telemetry_config(t: &TelemetryConfig) -> Result<(), ConfigError> {
    if t.service_name.is_empty() {
        return Err(ConfigError::InvalidServiceName);
    }
    if !VALID_LOG_LEVELS.contains(&t.log_level.as_str()) {
        return Err(ConfigError::InvalidLogLevel(t.log_level.clone()));
    }
    if !VALID_LOG_FORMATS.contains(&t.log_format.as_str()) {
        return Err(ConfigError::InvalidLogFormat(t.log_format.clone()));
    }
    Ok(())
}

#[cfg(test)]
#[path = "../test_helpers/config_schema_test_helpers.rs"]
mod test_helpers;
#[cfg(test)]
#[path = "../tests/config_schema_tests.rs"]
mod tests;
