#[path = "../../test_support/config_sections.rs"]
mod config_sections;

pub use self::config_sections::{database_section, telemetry_section};
use self::config_sections::{health_check_section, server_section};

const HOST: &str = "0.0.0.0";
const PORT: u16 = 9229;
const DRAIN_TIMEOUT_SECS: u64 = 20;
const HEALTH_CHECK_HOST: &str = "127.0.0.1";
const CONNECT_TIMEOUT_SECS: u64 = 2;
const INFO_LEVEL: &str = "info";

pub fn valid_yaml() -> String {
    format!(
        "{}{}{}",
        server_section(HOST, PORT, DRAIN_TIMEOUT_SECS),
        telemetry_section(INFO_LEVEL),
        health_check_section(HEALTH_CHECK_HOST, CONNECT_TIMEOUT_SECS),
    )
}

pub fn write_valid_config() -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("config.yaml");
    std::fs::write(&path, valid_yaml()).expect("write config");
    let path_str = path.to_str().expect("valid path").to_string();
    (dir, path_str)
}
