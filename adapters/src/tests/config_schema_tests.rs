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
fn validate_rejects_a_zero_log_read_max_bytes() {
    let mut cfg = valid_config();
    cfg.pm3.log_read_max_bytes = 0;
    let err = validate_config(&cfg).unwrap_err();
    assert!(
        matches!(err, ConfigError::InvalidLogReadMaxBytes(0)),
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
fn validate_rejects_zero_max_restart_delay() {
    let mut cfg = valid_config();
    cfg.pm3.restart.max_restart_delay_ms = 0;
    let err = validate_config(&cfg).unwrap_err();
    assert!(
        matches!(err, ConfigError::InvalidMaxRestartDelay(0)),
        "got: {err}"
    );
}

#[test]
fn validate_rejects_zero_log_rotate_interval() {
    let mut cfg = valid_config();
    cfg.pm3.log_rotate_interval_ms = 0;
    let err = validate_config(&cfg).unwrap_err();
    assert!(
        matches!(err, ConfigError::InvalidLogRotateInterval(0)),
        "got: {err}"
    );
}

#[test]
fn validate_rejects_zero_ready_timeout() {
    let mut cfg = valid_config();
    cfg.pm3.ready_timeout_ms = 0;
    let err = validate_config(&cfg).unwrap_err();
    assert!(
        matches!(err, ConfigError::InvalidReadyTimeout(0)),
        "got: {err}"
    );
}

#[test]
fn validate_rejects_zero_ready_poll_interval() {
    let mut cfg = valid_config();
    cfg.pm3.ready_poll_interval_ms = 0;
    let err = validate_config(&cfg).unwrap_err();
    assert!(
        matches!(err, ConfigError::InvalidReadyPollInterval(0)),
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

#[path = "config_schema_render_tests.rs"]
mod render;
