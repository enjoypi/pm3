use super::{test_helpers::*, *};

#[test]
fn show_config_valid() {
    let (_dir, path) = write_valid_config();
    let yaml = show_config(&path).expect("should succeed");
    assert!(yaml.contains("host"));
    assert!(yaml.contains("0.0.0.0"));
    assert!(yaml.contains("9229"));
}

#[test]
fn show_config_roundtrip() {
    let (_dir, path) = write_valid_config();
    let yaml = show_config(&path).expect("should succeed");
    let dir2 = tempfile::tempdir().expect("create temp dir");
    let path2 = dir2.path().join("roundtrip.yaml");
    std::fs::write(&path2, &yaml).expect("write roundtrip config");
    let reparsed = load_and_parse_config(path2.to_str().expect("path"))
        .expect("should reparse and revalidate");
    let server = reparsed.server.as_ref().expect("server present");
    assert_eq!(server.host, "0.0.0.0");
    assert_eq!(server.port, 9229);
}

#[test]
fn show_config_redacts_database_credentials() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("config.yaml");
    let config_yaml = format!(
        "{}{}",
        telemetry_section("info"),
        database_section("postgres://app:supersecret@db:5432/app", "./migrations", 5),
    );
    std::fs::write(&path, config_yaml).expect("write config");
    let yaml = show_config(path.to_str().expect("valid path")).expect("should succeed");
    assert!(!yaml.contains("supersecret"), "got: {yaml}");
    assert!(yaml.contains("postgres://***@db:5432/app"), "got: {yaml}");
}

#[test]
fn show_config_missing_file() {
    let result = show_config("/nonexistent/config.yaml");
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("config file"), "got: {err}");
}

#[test]
fn show_config_invalid_config() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("bad.yaml");
    std::fs::write(&path, "server:\n  host: localhost\n").expect("write");
    let result = show_config(path.to_str().expect("path"));
    assert!(result.is_err());
}

#[test]
fn load_and_parse_config_valid() {
    let (_dir, path) = write_valid_config();
    let cfg = load_and_parse_config(&path).expect("should succeed");
    assert_eq!(cfg.server.as_ref().expect("server present").host, "0.0.0.0");
}

#[test]
fn load_and_parse_config_missing_file() {
    let result = load_and_parse_config("/nonexistent/config.yaml");
    assert!(result.is_err());
}

#[test]
fn parse_valid_config() {
    let cfg = parse_config(&valid_yaml()).expect("should parse");
    let server = cfg.server.as_ref().expect("server present");
    assert_eq!(server.host, "0.0.0.0");
    assert_eq!(server.port, 9229);
    assert_eq!(server.drain_timeout_secs, 20);
    assert_eq!(cfg.telemetry.service_name, "skel_rs");
}

#[test]
fn parse_config_without_server() {
    let yaml = r#"
telemetry:
  service_name: "skel_rs"
  log_level: "info"
  log_format: "json"
"#;
    let cfg = parse_config(yaml).expect("should parse without server section");
    assert!(cfg.server.is_none());
}

#[test]
fn parse_invalid_yaml_syntax() {
    let result = parse_config("{{invalid yaml");
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("cannot parse config"), "got: {err}");
}

#[test]
fn parse_missing_fields() {
    let result = parse_config("server:\n  host: localhost\n");
    assert!(result.is_err());
}

#[test]
fn parse_custom_drain_timeout() {
    let yaml = r#"
server:
  host: "0.0.0.0"
  port: 9229
  drain_timeout_secs: 30
telemetry:
  service_name: "skel_rs"
  log_level: "info"
  log_format: "json"
health_check:
  host: "127.0.0.1"
  connect_timeout_secs: 2
"#;
    let cfg = parse_config(yaml).expect("should parse");
    assert_eq!(
        cfg.server
            .as_ref()
            .expect("server present")
            .drain_timeout_secs,
        30
    );
}

#[test]
fn parse_config_without_database() {
    let cfg = parse_config(&valid_yaml()).expect("should parse");
    assert!(cfg.database.is_none());
}

#[test]
fn parse_config_with_database() {
    let yaml = format!(
        "{}{}",
        valid_yaml(),
        database_section("sqlite://test.db?mode=rwc", "./migrations", 5),
    );
    let cfg = parse_config(&yaml).expect("should parse");
    let db = cfg.database.as_ref().expect("database should be present");
    assert_eq!(db.url, "sqlite://test.db?mode=rwc");
    assert_eq!(db.migrations_path, "./migrations");
    assert_eq!(db.pool.max_connections, 5);
    assert_eq!(db.pool.min_connections, 1);
    assert_eq!(db.pool.acquire_timeout_secs, 5);
}

#[test]
fn parse_database_missing_pool_fields_fails() {
    let yaml = format!(
        r#"{}
database:
  url: "sqlite://test.db"
"#,
        valid_yaml()
    );
    let result = parse_config(&yaml);
    assert!(result.is_err(), "should fail when pool fields are missing");
}
