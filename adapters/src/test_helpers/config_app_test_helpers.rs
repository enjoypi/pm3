use usecases::SandboxMode;

pub use crate::config_sections::{pm3_section, telemetry_section};

pub const HOME: &str = "/tmp/pm3-test-home";
pub const KILL_TIMEOUT_MS: u64 = 1600;
pub const SANDBOX_MODE: &str = SandboxMode::WorkspaceWrite.as_str();
const INFO_LEVEL: &str = "info";

pub fn valid_yaml() -> String {
    format!(
        "{}{}",
        pm3_section(HOME, KILL_TIMEOUT_MS, SANDBOX_MODE),
        telemetry_section(INFO_LEVEL),
    )
}

pub fn write_valid_config() -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("config.yaml");
    std::fs::write(&path, valid_yaml()).expect("write config");
    let path_str = path.to_str().expect("valid path").to_string();
    (dir, path_str)
}
