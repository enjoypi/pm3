use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("cannot parse config: {0}")]
    ParseError(String),

    #[error("cannot accept empty pm3.home")]
    InvalidHome,

    #[error("cannot accept pm3.kill_timeout_ms {0}: must be >= 1")]
    InvalidKillTimeout(u64),

    #[error("cannot accept pm3.start_timeout_ms {0}: must be >= 1")]
    InvalidStartTimeout(u64),

    #[error("cannot accept pm3.drain_timeout_secs {0}: must be >= 1")]
    InvalidDrainTimeout(u64),

    #[error("cannot accept pm3.daemon_poll_interval_ms {0}: must be >= 1")]
    InvalidPollInterval(u64),

    #[error("cannot accept pm3.restart.min_uptime_ms {0}: must be >= 1")]
    InvalidMinUptime(u64),

    #[error(
        "cannot accept pm3.sandbox.mode {0}: must be one of read-only, workspace-write, danger-full-access"
    )]
    InvalidSandboxMode(String),

    #[error("cannot accept empty pm3.service.label")]
    InvalidServiceLabel,

    #[error("cannot accept empty pm3.service.search_path")]
    InvalidServiceSearchPath,

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
    pub kill_timeout_ms: u64,
    pub start_timeout_ms: u64,
    pub drain_timeout_secs: u64,
    pub daemon_poll_interval_ms: u64,
    pub restart: RestartConfig,
    pub sandbox: SandboxConfig,
    pub service: ServiceConfig,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct RestartConfig {
    pub min_uptime_ms: u64,
    pub max_restarts: u32,
    pub restart_delay_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SandboxConfig {
    pub mode: String,
    pub network: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ServiceConfig {
    pub label: String,
    pub search_path: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TelemetryConfig {
    pub service_name: String,
    pub log_level: String,
    pub log_format: String,
}

pub const LOG_FORMAT_JSON: &str = "json";
pub const LOG_FORMAT_PRETTY: &str = "pretty";

pub const SANDBOX_MODE_READ_ONLY: &str = "read-only";
pub const SANDBOX_MODE_WORKSPACE_WRITE: &str = "workspace-write";
pub const SANDBOX_MODE_DANGER_FULL_ACCESS: &str = "danger-full-access";

const VALID_LOG_LEVELS: &[&str] = &["trace", "debug", "info", "warn", "error"];
const VALID_LOG_FORMATS: &[&str] = &[LOG_FORMAT_JSON, LOG_FORMAT_PRETTY];
const VALID_SANDBOX_MODES: &[&str] = &[
    SANDBOX_MODE_READ_ONLY,
    SANDBOX_MODE_WORKSPACE_WRITE,
    SANDBOX_MODE_DANGER_FULL_ACCESS,
];

pub fn validate_config(cfg: &AppConfig) -> Result<(), ConfigError> {
    validate_pm3_config(&cfg.pm3)?;
    validate_telemetry_config(&cfg.telemetry)
}

pub fn validate_pm3_config(pm3: &Pm3Config) -> Result<(), ConfigError> {
    if pm3.home.is_empty() {
        return Err(ConfigError::InvalidHome);
    }
    if pm3.kill_timeout_ms < 1 {
        return Err(ConfigError::InvalidKillTimeout(pm3.kill_timeout_ms));
    }
    if pm3.start_timeout_ms < 1 {
        return Err(ConfigError::InvalidStartTimeout(pm3.start_timeout_ms));
    }
    if pm3.drain_timeout_secs < 1 {
        return Err(ConfigError::InvalidDrainTimeout(pm3.drain_timeout_secs));
    }
    if pm3.daemon_poll_interval_ms < 1 {
        return Err(ConfigError::InvalidPollInterval(
            pm3.daemon_poll_interval_ms,
        ));
    }
    if pm3.restart.min_uptime_ms < 1 {
        return Err(ConfigError::InvalidMinUptime(pm3.restart.min_uptime_ms));
    }
    if !VALID_SANDBOX_MODES.contains(&pm3.sandbox.mode.as_str()) {
        return Err(ConfigError::InvalidSandboxMode(pm3.sandbox.mode.clone()));
    }
    if pm3.service.label.is_empty() {
        return Err(ConfigError::InvalidServiceLabel);
    }
    if pm3.service.search_path.is_empty() {
        return Err(ConfigError::InvalidServiceSearchPath);
    }
    Ok(())
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
