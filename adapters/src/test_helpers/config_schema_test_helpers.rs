use super::*;

pub fn valid_restart_config() -> RestartConfig {
    RestartConfig {
        min_uptime_ms: 1000,
        max_restarts: 15,
        restart_delay_ms: 0,
    }
}

pub fn valid_sandbox_config() -> SandboxConfig {
    SandboxConfig {
        mode: SANDBOX_MODE_WORKSPACE_WRITE.to_string(),
        network: false,
    }
}

pub fn valid_service_config() -> ServiceConfig {
    ServiceConfig {
        label: "pm3-fixture".to_string(),
    }
}

pub fn valid_pm3_config() -> Pm3Config {
    Pm3Config {
        home: "/tmp/pm3-fixture".to_string(),
        cfg_dir: "/tmp/pm3-fixture/svc".to_string(),
        search_path: "/usr/bin:/bin".to_string(),
        stop_signal: STOP_SIGNAL_TERM.to_string(),
        kill_timeout_ms: 1600,
        start_timeout_ms: 5000,
        drain_timeout_secs: 5,
        request_timeout_ms: 30000,
        command_timeout_ms: 5000,
        daemon_poll_interval_ms: 50,
        log_follow_interval_ms: 200,
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
