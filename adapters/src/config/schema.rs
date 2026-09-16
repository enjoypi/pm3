use serde::{Deserialize, Serialize};
use thiserror::Error;

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

    #[error("cannot accept pm3.sops_timeout_ms {0}: must be >= 1")]
    InvalidSopsTimeout(u64),

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

    #[error("cannot accept pm3.liveness_poll_interval_ms {0}: must be >= 1")]
    InvalidLivenessPollInterval(u64),

    #[error("cannot accept pm3.liveness_failure_threshold {0}: must be >= 1")]
    InvalidLivenessThreshold(u32),

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
    #[serde(default = "default_liveness_poll_interval_ms")]
    pub liveness_poll_interval_ms: u64,

    #[serde(default = "default_liveness_failure_threshold")]
    pub liveness_failure_threshold: u32,
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
    #[serde(default)]
    pub sops_identity_file: String,
    #[serde(default = "default_sops_program")]
    pub sops_program: String,
    #[serde(default = "default_sops_timeout_ms")]
    pub sops_timeout_ms: u64,
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
pub const DEFAULT_LIVENESS_POLL_INTERVAL_MS: u64 = 30000;
pub const DEFAULT_LIVENESS_FAILURE_THRESHOLD: u32 = 3;
pub const DEFAULT_SOPS_TIMEOUT_MS: u64 = 5000;

fn default_sops_program() -> String {
    crate::apps_file::SOPS_PROGRAM.to_string()
}

const fn default_sops_timeout_ms() -> u64 {
    DEFAULT_SOPS_TIMEOUT_MS
}

const fn default_liveness_poll_interval_ms() -> u64 {
    DEFAULT_LIVENESS_POLL_INTERVAL_MS
}

const fn default_liveness_failure_threshold() -> u32 {
    DEFAULT_LIVENESS_FAILURE_THRESHOLD
}

const fn default_log_read_max_bytes() -> u64 {
    DEFAULT_LOG_READ_MAX_BYTES
}

pub const LOG_FORMAT_JSON: &str = "json";
pub const LOG_FORMAT_PRETTY: &str = "pretty";

pub const STOP_SIGNAL_TERM: &str = "TERM";

pub const RESTART_CONDITION_ALWAYS: &str = "always";
pub const RESTART_CONDITION_ON_FAILURE: &str = "on-failure";
