use super::{test_helpers::*, *};

#[test]
fn validate_accepts_valid_config() {
    validate_config(&valid_config()).expect("fixture should validate");
}

#[test]
fn validate_rejects_empty_home() {
    let mut cfg = valid_config();
    cfg.pm3.home = String::new();
    let err = validate_config(&cfg).unwrap_err();
    assert!(matches!(err, ConfigError::InvalidHome), "got: {err}");
}

#[test]
fn validate_rejects_zero_kill_timeout() {
    let mut cfg = valid_config();
    cfg.pm3.kill_timeout_ms = 0;
    let err = validate_config(&cfg).unwrap_err();
    assert!(
        matches!(err, ConfigError::InvalidKillTimeout(0)),
        "got: {err}"
    );
}

#[test]
fn validate_rejects_zero_start_timeout() {
    let mut cfg = valid_config();
    cfg.pm3.start_timeout_ms = 0;
    let err = validate_config(&cfg).unwrap_err();
    assert!(
        matches!(err, ConfigError::InvalidStartTimeout(0)),
        "got: {err}"
    );
}

#[test]
fn validate_rejects_zero_drain_timeout() {
    let mut cfg = valid_config();
    cfg.pm3.drain_timeout_secs = 0;
    let err = validate_config(&cfg).unwrap_err();
    assert!(
        matches!(err, ConfigError::InvalidDrainTimeout(0)),
        "got: {err}"
    );
}

#[test]
fn validate_rejects_a_zero_daemon_poll_interval() {
    let mut cfg = valid_config();
    cfg.pm3.daemon_poll_interval_ms = 0;
    let err = validate_config(&cfg).unwrap_err();
    assert!(
        matches!(err, ConfigError::InvalidPollInterval(0)),
        "got: {err}"
    );
}

#[test]
fn validate_rejects_zero_min_uptime() {
    let mut cfg = valid_config();
    cfg.pm3.restart.min_uptime_ms = 0;
    let err = validate_config(&cfg).unwrap_err();
    assert!(
        matches!(err, ConfigError::InvalidMinUptime(0)),
        "got: {err}"
    );
}

#[test]
fn validate_accepts_zero_max_restarts() {
    let mut cfg = valid_config();
    cfg.pm3.restart.max_restarts = 0;
    validate_config(&cfg).expect("zero max_restarts means give up on first crash");
}

#[test]
fn validate_rejects_unknown_sandbox_mode() {
    let mut cfg = valid_config();
    cfg.pm3.sandbox.mode = "yolo".to_string();
    let err = validate_config(&cfg).unwrap_err();
    assert!(
        matches!(err, ConfigError::InvalidSandboxMode(_)),
        "got: {err}"
    );
}

#[test]
fn validate_accepts_every_sandbox_mode() {
    for mode in [
        SANDBOX_MODE_READ_ONLY,
        SANDBOX_MODE_WORKSPACE_WRITE,
        SANDBOX_MODE_DANGER_FULL_ACCESS,
    ] {
        let mut cfg = valid_config();
        cfg.pm3.sandbox.mode = mode.to_string();
        validate_config(&cfg).expect("documented sandbox mode should validate");
    }
}

#[test]
fn validate_rejects_an_empty_service_label() {
    let mut cfg = valid_config();
    cfg.pm3.service.label = String::new();
    let err = validate_config(&cfg).unwrap_err();
    assert!(
        matches!(err, ConfigError::InvalidServiceLabel),
        "got: {err}"
    );
}

#[test]
fn validate_rejects_an_empty_search_path() {
    let mut cfg = valid_config();
    cfg.pm3.search_path = String::new();
    let err = validate_config(&cfg).unwrap_err();
    assert!(matches!(err, ConfigError::InvalidSearchPath), "got: {err}");
}

#[test]
fn validate_rejects_empty_service_name() {
    let mut cfg = valid_config();
    cfg.telemetry.service_name = String::new();
    let err = validate_config(&cfg).unwrap_err();
    assert!(matches!(err, ConfigError::InvalidServiceName), "got: {err}");
}

#[test]
fn validate_rejects_unknown_log_level() {
    let mut cfg = valid_config();
    cfg.telemetry.log_level = "verbose".to_string();
    let err = validate_config(&cfg).unwrap_err();
    assert!(matches!(err, ConfigError::InvalidLogLevel(_)), "got: {err}");
}

#[test]
fn validate_rejects_unknown_log_format() {
    let mut cfg = valid_config();
    cfg.telemetry.log_format = "xml".to_string();
    let err = validate_config(&cfg).unwrap_err();
    assert!(
        matches!(err, ConfigError::InvalidLogFormat(_)),
        "got: {err}"
    );
}

#[test]
fn validate_telemetry_accepts_pretty_format() {
    let telemetry = TelemetryConfig {
        log_format: LOG_FORMAT_PRETTY.to_string(),
        ..valid_telemetry_config()
    };
    validate_telemetry_config(&telemetry).expect("pretty is a documented format");
}

#[test]
fn validate_pm3_config_direct_accepts_fixture() {
    validate_pm3_config(&valid_pm3_config()).expect("fixture should validate");
}

#[test]
fn parse_error_renders_reason() {
    let err = ConfigError::ParseError("bad yaml".to_string());
    assert_eq!(err.to_string(), "cannot parse config: bad yaml");
}

#[test]
fn every_error_variant_renders_a_message() {
    let errors = [
        ConfigError::InvalidHome,
        ConfigError::InvalidCfgDir,
        ConfigError::InvalidKillTimeout(0),
        ConfigError::InvalidStartTimeout(0),
        ConfigError::InvalidDrainTimeout(0),
        ConfigError::InvalidMinUptime(0),
        ConfigError::InvalidSandboxMode("yolo".to_string()),
        ConfigError::InvalidServiceLabel,
        ConfigError::InvalidSearchPath,
        ConfigError::InvalidServiceName,
        ConfigError::InvalidLogLevel("verbose".to_string()),
        ConfigError::InvalidLogFormat("xml".to_string()),
    ];
    for err in errors {
        assert!(
            err.to_string().starts_with("cannot accept"),
            "error message must start with a verb: {err}"
        );
    }
}

#[test]
fn validate_rejects_an_empty_cfg_dir() {
    let mut cfg = valid_config();
    cfg.pm3.cfg_dir = String::new();
    let err = validate_config(&cfg).unwrap_err();
    assert!(matches!(err, ConfigError::InvalidCfgDir), "got: {err}");
}
