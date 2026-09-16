use super::*;

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
fn validate_rejects_a_task_cap_that_leaves_no_room_for_the_daemon() {
    let mut cfg = valid_config();
    cfg.pm3.service.max_tasks = 0;
    let err = validate_config(&cfg).unwrap_err();
    assert!(matches!(err, ConfigError::InvalidMaxTasks(0)), "got: {err}");
}

#[test]
fn validate_rejects_a_zero_request_body_limit() {
    let mut cfg = valid_config();
    cfg.pm3.request_body_limit_bytes = 0;
    let err = validate_config(&cfg).unwrap_err();
    assert!(
        matches!(err, ConfigError::InvalidBodyLimit(0)),
        "got: {err}"
    );
}

#[test]
fn validate_rejects_a_zero_sops_timeout() {
    let mut cfg = valid_config();
    cfg.pm3.sops_timeout_ms = 0;
    let err = validate_config(&cfg).unwrap_err();
    assert!(
        matches!(err, ConfigError::InvalidSopsTimeout(0)),
        "a zero deadline would refuse every decryption, got: {err}"
    );
}

#[test]
fn validate_rejects_an_empty_sops_program() {
    let mut cfg = valid_config();
    cfg.pm3.sops_program = String::new();
    let err = validate_config(&cfg).unwrap_err();
    assert_eq!(err.to_string(), "cannot accept empty pm3.sops_program");
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
fn validate_rejects_an_empty_schtasks_path() {
    let mut cfg = valid_config();
    cfg.pm3.service.schtasks_path = String::new();
    let err = validate_config(&cfg).unwrap_err();
    assert_eq!(
        err.to_string(),
        "cannot accept empty pm3.service.schtasks_path"
    );
}

#[test]
fn validate_rejects_an_empty_taskkill_path() {
    let mut cfg = valid_config();
    cfg.pm3.service.taskkill_path = String::new();
    let err = validate_config(&cfg).unwrap_err();
    assert_eq!(
        err.to_string(),
        "cannot accept empty pm3.service.taskkill_path"
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
        ConfigError::InvalidRoot {
            field: "pm3.state_dir",
        },
        ConfigError::InvalidKillTimeout(0),
        ConfigError::InvalidStartTimeout(0),
        ConfigError::InvalidDrainTimeout(0),
        ConfigError::InvalidRequestTimeout(0),
        ConfigError::InvalidCommandTimeout(0),
        ConfigError::InvalidPollInterval(0),
        ConfigError::InvalidPollCeiling { max: 1, floor: 2 },
        ConfigError::InvalidFollowInterval(0),
        ConfigError::InvalidLogTailLines(0),
        ConfigError::InvalidLogReadMaxBytes(0),
        ConfigError::InvalidLogRotateInterval(0),
        ConfigError::InvalidReadyTimeout(0),
        ConfigError::InvalidReadyPollInterval(0),
        ConfigError::InvalidChannelDepth(0),
        ConfigError::InvalidBodyLimit(0),
        ConfigError::InvalidSopsTimeout(0),
        ConfigError::InvalidMaxTasks(0),
        ConfigError::EmptyProgram {
            field: "pm3.sandbox.bwrap_program",
        },
        ConfigError::InvalidRestartCondition("sometimes".to_string()),
        ConfigError::InvalidStopSignal("BOOM".to_string()),
        ConfigError::InvalidMinUptime(0),
        ConfigError::InvalidMaxRestartDelay(0),
        ConfigError::InvalidSandboxMode {
            mode: "yolo".to_string(),
            expected: "read-only, workspace-write, danger-full-access".to_string(),
        },
        ConfigError::InvalidMemoryPollInterval(0),
        ConfigError::InvalidLivenessPollInterval(0),
        ConfigError::InvalidLivenessThreshold(0),
        ConfigError::InvalidSandboxRead {
            read: "everything".to_string(),
            expected: "full, minimal".to_string(),
        },
        ConfigError::RelativeSandboxRoot {
            field: "pm3.sandbox.minimal_read_roots",
            root: "usr".to_string(),
        },
        ConfigError::EmptyMinimalReadRoots,
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
fn validate_accepts_an_empty_cfg_dir_because_it_follows_the_config_root() {
    let mut cfg = valid_config();
    cfg.pm3.cfg_dir = String::new();
    validate_config(&cfg).expect("an empty cfg_dir follows the config root");
}

#[test]
fn validate_rejects_a_relative_cfg_dir() {
    let mut cfg = valid_config();
    cfg.pm3.cfg_dir = "service".to_string();
    let err = validate_config(&cfg).unwrap_err();
    assert!(matches!(err, ConfigError::InvalidCfgDir), "got: {err}");
}

#[test]
fn validate_rejects_a_zero_memory_poll_interval() {
    let mut cfg = valid_config();
    cfg.pm3.memory_poll_interval_ms = 0;
    let err = validate_config(&cfg).unwrap_err();
    assert!(
        matches!(err, ConfigError::InvalidMemoryPollInterval(0)),
        "got: {err}"
    );
}

#[test]
fn validate_rejects_a_zero_liveness_poll_interval() {
    let mut cfg = valid_config();
    cfg.pm3.liveness_poll_interval_ms = 0;
    let err = validate_config(&cfg).unwrap_err();
    assert!(
        matches!(err, ConfigError::InvalidLivenessPollInterval(0)),
        "got: {err}"
    );
}

#[test]
fn validate_rejects_a_zero_liveness_threshold() {
    let mut cfg = valid_config();
    cfg.pm3.liveness_failure_threshold = 0;
    let err = validate_config(&cfg).unwrap_err();
    assert!(
        matches!(err, ConfigError::InvalidLivenessThreshold(0)),
        "got: {err}"
    );
}

#[path = "config_validate_sandbox_tests.rs"]
mod sandbox_roots;

#[derive(serde::Deserialize)]
struct Pm3Head {
    pm3: Pm3Config,
}

#[test]
fn a_config_written_before_liveness_existed_still_parses() {
    let text = crate::config_sections::pm3_section("/tmp/pm3-bw", 1600, "read-only");
    let kept: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim_start().starts_with("liveness_"))
        .collect();
    let pruned = kept.join("\n");
    let pm3 = serde_yaml2::from_str::<Pm3Head>(&pruned)
        .expect("an older config still parses")
        .pm3;
    assert_eq!(pm3.liveness_poll_interval_ms, 30000);
    assert_eq!(pm3.liveness_failure_threshold, 3);
}
