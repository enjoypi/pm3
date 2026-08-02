use usecases::SandboxMode;

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
fn validate_rejects_a_zero_request_timeout() {
    let mut cfg = valid_config();
    cfg.pm3.request_timeout_ms = 0;
    let err = validate_config(&cfg).unwrap_err();
    assert!(
        matches!(err, ConfigError::InvalidRequestTimeout(0)),
        "got: {err}"
    );
}

#[test]
fn validate_rejects_a_zero_command_timeout() {
    let mut cfg = valid_config();
    cfg.pm3.command_timeout_ms = 0;
    let err = validate_config(&cfg).unwrap_err();
    assert!(
        matches!(err, ConfigError::InvalidCommandTimeout(0)),
        "got: {err}"
    );
}

#[test]
fn validate_rejects_a_zero_log_follow_interval() {
    let mut cfg = valid_config();
    cfg.pm3.log_follow_interval_ms = 0;
    let err = validate_config(&cfg).unwrap_err();
    assert!(
        matches!(err, ConfigError::InvalidFollowInterval(0)),
        "got: {err}"
    );
}

#[test]
fn validate_rejects_a_zero_log_tail_lines() {
    let mut cfg = valid_config();
    cfg.pm3.log_tail_lines = 0;
    let err = validate_config(&cfg).unwrap_err();
    assert!(
        matches!(err, ConfigError::InvalidLogTailLines(0)),
        "got: {err}"
    );
}

#[test]
fn validate_rejects_an_unknown_stop_signal() {
    let mut cfg = valid_config();
    cfg.pm3.stop_signal = "BOOM".to_string();
    let err = validate_config(&cfg).unwrap_err();
    assert!(
        matches!(err, ConfigError::InvalidStopSignal(_)),
        "got: {err}"
    );
}

#[test]
fn validate_accepts_sigint_as_a_stop_signal() {
    let mut cfg = valid_config();
    cfg.pm3.stop_signal = "INT".to_string();
    validate_config(&cfg).expect("pm2 users stopping with SIGINT should be able to say so");
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
fn validate_rejects_a_poll_ceiling_below_the_poll_interval() {
    let mut cfg = valid_config();
    cfg.pm3.daemon_poll_interval_ms = 500;
    cfg.pm3.daemon_poll_max_interval_ms = 100;
    let err = validate_config(&cfg).unwrap_err();
    assert!(
        matches!(
            err,
            ConfigError::InvalidPollCeiling {
                max: 100,
                floor: 500
            }
        ),
        "got: {err}"
    );
}

#[test]
fn validate_accepts_a_poll_ceiling_equal_to_the_poll_interval() {
    let mut cfg = valid_config();
    cfg.pm3.daemon_poll_max_interval_ms = cfg.pm3.daemon_poll_interval_ms;
    validate_config(&cfg).expect("a flat cadence is a legitimate choice");
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
        matches!(err, ConfigError::InvalidSandboxMode { .. }),
        "got: {err}"
    );
}

#[test]
fn an_invalid_sandbox_mode_lists_the_modes_the_domain_knows() {
    let mut cfg = valid_config();
    cfg.pm3.sandbox.mode = "yolo".to_string();
    let err = validate_config(&cfg).unwrap_err().to_string();
    assert_eq!(
        err,
        "cannot accept pm3.sandbox.mode yolo: must be one of read-only, workspace-write, danger-full-access"
    );
}

#[test]
fn validate_accepts_every_sandbox_mode() {
    for mode in SandboxMode::ALL {
        let mut cfg = valid_config();
        cfg.pm3.sandbox.mode = mode.as_str().to_string();
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
fn validate_rejects_a_service_label_with_a_path_separator() {
    let mut cfg = valid_config();
    cfg.pm3.service.label = "team/pm3".to_string();
    let err = validate_config(&cfg).unwrap_err();
    assert!(
        matches!(err, ConfigError::UnsafeServiceLabel { .. }),
        "got: {err}"
    );
}

#[test]
fn validate_rejects_a_service_label_with_a_control_character() {
    let mut cfg = valid_config();
    cfg.pm3.service.label = "pm3\nWantedBy=evil.target".to_string();
    let err = validate_config(&cfg).unwrap_err();
    assert!(
        matches!(err, ConfigError::UnsafeServiceLabel { .. }),
        "got: {err}"
    );
}

#[test]
fn validate_rejects_a_service_label_starting_with_a_dot() {
    let mut cfg = valid_config();
    cfg.pm3.service.label = ".pm3".to_string();
    let err = validate_config(&cfg).unwrap_err();
    assert!(
        matches!(err, ConfigError::DottedServiceLabel(_)),
        "got: {err}"
    );
}

#[test]
fn validate_rejects_a_home_with_a_line_break() {
    let mut cfg = valid_config();
    cfg.pm3.home = "/home/dev\nWantedBy=evil.target".to_string();
    let err = validate_config(&cfg).unwrap_err();
    assert!(
        matches!(err, ConfigError::UnsafeLineBreak { .. }),
        "got: {err}"
    );
}

#[test]
fn validate_rejects_a_search_path_with_a_line_break() {
    let mut cfg = valid_config();
    cfg.pm3.search_path = "/usr/bin\nWantedBy=evil.target".to_string();
    let err = validate_config(&cfg).unwrap_err();
    assert!(
        matches!(err, ConfigError::UnsafeLineBreak { .. }),
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
fn validate_rejects_a_zero_daemon_channel_depth() {
    let mut cfg = valid_config();
    cfg.pm3.daemon_channel_depth = 0;
    let err = validate_config(&cfg).unwrap_err();
    assert!(
        matches!(err, ConfigError::InvalidChannelDepth(0)),
        "got: {err}"
    );
}

#[test]
fn validate_rejects_an_empty_seatbelt_program() {
    let mut cfg = valid_config();
    cfg.pm3.sandbox.seatbelt_program = String::new();
    let err = validate_config(&cfg).unwrap_err();
    assert_eq!(
        err.to_string(),
        "cannot accept empty pm3.sandbox.seatbelt_program"
    );
}

#[test]
fn validate_rejects_an_empty_bwrap_program() {
    let mut cfg = valid_config();
    cfg.pm3.sandbox.bwrap_program = String::new();
    let err = validate_config(&cfg).unwrap_err();
    assert_eq!(
        err.to_string(),
        "cannot accept empty pm3.sandbox.bwrap_program"
    );
}

#[test]
fn validate_rejects_an_empty_launchctl_path() {
    let mut cfg = valid_config();
    cfg.pm3.service.launchctl_path = String::new();
    let err = validate_config(&cfg).unwrap_err();
    assert_eq!(
        err.to_string(),
        "cannot accept empty pm3.service.launchctl_path"
    );
}

#[test]
fn validate_rejects_an_empty_systemctl_path() {
    let mut cfg = valid_config();
    cfg.pm3.service.systemctl_path = String::new();
    let err = validate_config(&cfg).unwrap_err();
    assert_eq!(
        err.to_string(),
        "cannot accept empty pm3.service.systemctl_path"
    );
}

#[test]
fn validate_rejects_an_empty_loginctl_path() {
    let mut cfg = valid_config();
    cfg.pm3.service.loginctl_path = String::new();
    let err = validate_config(&cfg).unwrap_err();
    assert_eq!(
        err.to_string(),
        "cannot accept empty pm3.service.loginctl_path"
    );
}

#[test]
fn validate_rejects_an_unknown_restart_condition() {
    let mut cfg = valid_config();
    cfg.pm3.service.restart_condition = "sometimes".to_string();
    let err = validate_config(&cfg).unwrap_err();
    assert!(
        matches!(err, ConfigError::InvalidRestartCondition(ref raw) if raw == "sometimes"),
        "got: {err}"
    );
}

#[test]
fn every_error_variant_renders_a_message() {
    let errors = [
        ConfigError::InvalidHome,
        ConfigError::InvalidCfgDir,
        ConfigError::InvalidKillTimeout(0),
        ConfigError::InvalidStartTimeout(0),
        ConfigError::InvalidDrainTimeout(0),
        ConfigError::InvalidRequestTimeout(0),
        ConfigError::InvalidCommandTimeout(0),
        ConfigError::InvalidPollInterval(0),
        ConfigError::InvalidPollCeiling { max: 1, floor: 2 },
        ConfigError::InvalidFollowInterval(0),
        ConfigError::InvalidLogTailLines(0),
        ConfigError::InvalidChannelDepth(0),
        ConfigError::EmptyProgram {
            field: "pm3.sandbox.bwrap_program",
        },
        ConfigError::InvalidRestartCondition("sometimes".to_string()),
        ConfigError::InvalidStopSignal("BOOM".to_string()),
        ConfigError::InvalidMinUptime(0),
        ConfigError::InvalidSandboxMode {
            mode: "yolo".to_string(),
            expected: "read-only, workspace-write, danger-full-access".to_string(),
        },
        ConfigError::InvalidServiceLabel,
        ConfigError::UnsafeServiceLabel {
            label: "a/b".to_string(),
            character: '/',
        },
        ConfigError::DottedServiceLabel(".pm3".to_string()),
        ConfigError::UnsafeLineBreak { field: "pm3.home" },
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
