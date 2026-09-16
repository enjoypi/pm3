use usecases::{ReadScope, SandboxMode, is_name_letter};

use super::schema::{
    AppConfig, ConfigError, LOG_FORMAT_JSON, LOG_FORMAT_PRETTY, Pm3Config,
    RESTART_CONDITION_ALWAYS, RESTART_CONDITION_ON_FAILURE, SandboxConfig, TelemetryConfig,
};
#[cfg(test)]
use super::schema::{RestartConfig, STOP_SIGNAL_TERM, ServiceConfig};

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

fn validate_budgets(pm3: &Pm3Config) -> Result<(), ConfigError> {
    validate_timeouts(pm3)?;
    validate_intervals(pm3)
}

const fn validate_timeouts(pm3: &Pm3Config) -> Result<(), ConfigError> {
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
    if pm3.ready_timeout_ms < 1 {
        return Err(ConfigError::InvalidReadyTimeout(pm3.ready_timeout_ms));
    }
    if pm3.sops_timeout_ms < 1 {
        return Err(ConfigError::InvalidSopsTimeout(pm3.sops_timeout_ms));
    }
    Ok(())
}

const fn validate_intervals(pm3: &Pm3Config) -> Result<(), ConfigError> {
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
    if pm3.liveness_poll_interval_ms < 1 {
        return Err(ConfigError::InvalidLivenessPollInterval(
            pm3.liveness_poll_interval_ms,
        ));
    }
    if pm3.liveness_failure_threshold < 1 {
        return Err(ConfigError::InvalidLivenessThreshold(
            pm3.liveness_failure_threshold,
        ));
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
    reject_empty("pm3.sops_program", &pm3.sops_program)?;
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
#[path = "../tests/config_validate_tests.rs"]
mod tests;
