use super::{test_helpers::*, *};

#[test]
fn show_config_renders_the_resolved_document() {
    let (_dir, path) = write_valid_config();
    let yaml = show_config(&path).expect("should succeed");
    assert!(yaml.contains("home"), "got: {yaml}");
    assert!(yaml.contains(HOME), "got: {yaml}");
    assert!(yaml.contains(SANDBOX_MODE), "got: {yaml}");
}

#[test]
fn show_config_output_parses_back_into_the_same_settings() {
    let (_dir, path) = write_valid_config();
    let yaml = show_config(&path).expect("should succeed");
    let dir = tempfile::tempdir().expect("create temp dir");
    let roundtrip = dir.path().join("roundtrip.yaml");
    std::fs::write(&roundtrip, &yaml).expect("write roundtrip config");
    let reparsed = load_and_parse_config(roundtrip.to_str().expect("path"))
        .expect("should reparse and revalidate");
    assert_eq!(reparsed.pm3.home, HOME);
    assert_eq!(reparsed.pm3.kill_timeout_ms, KILL_TIMEOUT_MS);
}

#[test]
fn show_config_reports_a_missing_file() {
    let err = show_config("/nonexistent/config.yaml")
        .unwrap_err()
        .to_string();
    assert!(err.contains("config file"), "got: {err}");
}

#[test]
fn show_config_rejects_an_incomplete_document() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("bad.yaml");
    std::fs::write(&path, "pm3:\n  home: /tmp/pm3\n").expect("write");
    assert!(show_config(path.to_str().expect("path")).is_err());
}

#[test]
fn load_and_parse_config_accepts_a_valid_file() {
    let (_dir, path) = write_valid_config();
    let cfg = load_and_parse_config(&path).expect("should succeed");
    assert_eq!(cfg.pm3.home, HOME);
}

#[test]
fn load_and_parse_config_reports_a_missing_file() {
    assert!(load_and_parse_config("/nonexistent/config.yaml").is_err());
}

#[test]
fn parse_config_reads_every_pm3_setting() {
    let cfg = parse_config(&valid_yaml()).expect("should parse");
    assert_eq!(cfg.pm3.home, HOME);
    assert_eq!(cfg.pm3.kill_timeout_ms, KILL_TIMEOUT_MS);
    assert_eq!(cfg.pm3.start_timeout_ms, 5000);
    assert_eq!(cfg.pm3.drain_timeout_secs, 5);
    assert_eq!(cfg.pm3.daemon_poll_interval_ms, 50);
    assert_eq!(cfg.pm3.restart.min_uptime_ms, 1000);
    assert_eq!(cfg.pm3.restart.max_restarts, 15);
    assert_eq!(cfg.pm3.restart.restart_delay_ms, 0);
    assert_eq!(cfg.pm3.sandbox.mode, SANDBOX_MODE);
    assert!(!cfg.pm3.sandbox.network);
    assert_eq!(cfg.telemetry.service_name, "pm3");
}

#[test]
fn parse_config_rejects_broken_yaml() {
    let err = parse_config("{{invalid yaml").unwrap_err().to_string();
    assert!(err.contains("cannot parse config"), "got: {err}");
}

#[test]
fn parse_config_rejects_a_document_without_the_pm3_section() {
    assert!(parse_config(&telemetry_section("info")).is_err());
}

#[test]
fn parse_config_rejects_a_document_without_telemetry() {
    let yaml = pm3_section(HOME, KILL_TIMEOUT_MS, SANDBOX_MODE);
    assert!(parse_config(&yaml).is_err());
}

#[test]
fn load_and_parse_config_rejects_an_invalid_sandbox_mode() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("config.yaml");
    let yaml = format!(
        "{}{}",
        pm3_section(HOME, KILL_TIMEOUT_MS, "yolo"),
        telemetry_section("info"),
    );
    std::fs::write(&path, yaml).expect("write config");
    let err = load_and_parse_config(path.to_str().expect("path"))
        .unwrap_err()
        .to_string();
    assert!(err.contains("pm3.sandbox.mode"), "got: {err}");
}

#[test]
fn load_and_parse_config_reports_an_unresolvable_placeholder() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("config.yaml");
    let yaml = format!(
        "{}{}",
        pm3_section("${PM3_TEST_UNSET_HOME}", KILL_TIMEOUT_MS, SANDBOX_MODE),
        telemetry_section("info"),
    );
    std::fs::write(&path, yaml).expect("write config");
    let err = load_and_parse_config(path.to_str().expect("path"))
        .unwrap_err()
        .to_string();
    assert!(err.contains("PM3_TEST_UNSET_HOME"), "got: {err}");
}

#[test]
fn check_config_confirms_a_valid_file() {
    let (_dir, path) = write_valid_config();
    let message = check_config(&path).expect("should succeed");
    assert!(message.contains(&path), "got: {message}");
}

#[test]
fn check_config_reports_an_invalid_file() {
    assert!(check_config("/nonexistent/config.yaml").is_err());
}

#[test]
fn environment_placeholders_are_substituted_before_parsing() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("config.yaml");
    let yaml = format!(
        "{}{}",
        pm3_section(
            "${PM3_TEST_HOME:-/tmp/pm3-default}",
            KILL_TIMEOUT_MS,
            SANDBOX_MODE
        ),
        telemetry_section("info"),
    );
    std::fs::write(&path, yaml).expect("write config");
    let cfg = load_and_parse_config(path.to_str().expect("path")).expect("should succeed");
    assert_eq!(cfg.pm3.home, "/tmp/pm3-default");
}
