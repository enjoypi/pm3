use usecases::SandboxMode;

use super::*;

pub fn valid_restart_config() -> RestartConfig {
    RestartConfig {
        autorestart: true,
        min_uptime_ms: 1000,
        max_restarts: 15,
        restart_delay_ms: 0,
    }
}

pub fn valid_sandbox_config() -> SandboxConfig {
    SandboxConfig {
        mode: SandboxMode::WorkspaceWrite.as_str().to_string(),
        network: false,
        seatbelt_program: "/usr/bin/sandbox-exec".to_string(),
        bwrap_program: "bwrap".to_string(),
    }
}

pub fn valid_service_config() -> ServiceConfig {
    ServiceConfig {
        label: "pm3-fixture".to_string(),
        restart_delay_secs: 2,
        restart_condition: RESTART_CONDITION_ALWAYS.to_string(),
        launchctl_path: "/bin/launchctl".to_string(),
        systemctl_path: "/usr/bin/systemctl".to_string(),
        loginctl_path: "/usr/bin/loginctl".to_string(),
    }
}

pub fn valid_pm3_config() -> Pm3Config {
    Pm3Config {
        home: "/tmp/pm3-fixture".to_string(),
        cfg_dir: "/tmp/pm3-fixture/service".to_string(),
        search_path: "/usr/bin:/bin".to_string(),
        stop_signal: STOP_SIGNAL_TERM.to_string(),
        kill_timeout_ms: 1600,
        start_timeout_ms: 5000,
        drain_timeout_secs: 5,
        request_timeout_ms: 30000,
        command_timeout_ms: 5000,
        daemon_poll_interval_ms: 50,
        daemon_poll_max_interval_ms: 1000,
        log_follow_interval_ms: 200,
        log_tail_lines: 20,
        daemon_channel_depth: 32,
        restart: valid_restart_config(),
        sandbox: valid_sandbox_config(),
        service: valid_service_config(),
    }
}

pub fn valid_telemetry_config() -> TelemetryConfig {
    TelemetryConfig {
        service_name: "pm3".to_string(),
        log_level: "info".to_string(),
        log_format: LOG_FORMAT_JSON.to_string(),
    }
}

pub fn valid_config() -> AppConfig {
    AppConfig {
        pm3: valid_pm3_config(),
        telemetry: valid_telemetry_config(),
    }
}
