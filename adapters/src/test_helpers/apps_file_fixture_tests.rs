use usecases::SandboxMode;

use super::*;
use crate::{
    apps_sections::{apps_section, every_optional_field_section},
    config_sections::fixture_config,
};

pub const APP_NAME: &str = "web";
pub const SCRIPT: &str = crate::platform::SHELL;
pub const CWD: &str = "/srv/web";
pub const HOME_DIR: &str = "/tmp/pm3-fixture";
pub const CFG_DIR: &str = "/tmp/pm3-fixture-cfg";
pub const LOGS_DIR: &str = "/tmp/pm3-fixture/logs";
pub const TMP_DIR: &str = "/tmp/pm3-fixture-tmp";

pub fn pm3_config(sandbox_mode: &str) -> Pm3Config {
    fixture_config(HOME_DIR, sandbox_mode).pm3
}

static FIXTURE_CONFIG: std::sync::LazyLock<Pm3Config> =
    std::sync::LazyLock::new(|| pm3_config(SandboxMode::WorkspaceWrite.as_str()));

pub fn fixture_roots() -> SpecRoots<'static> {
    SpecRoots {
        home_dir: HOME_DIR,
        cfg_dir: CFG_DIR,
        apps_dir: HOME_DIR,
        state_dir: HOME_DIR,
        runtime_dir: HOME_DIR,
        data_dir: HOME_DIR,
        logs_dir: LOGS_DIR,
        tmp_dir: Some(TMP_DIR),
    }
}

pub fn defaults() -> SpecDefaults<'static> {
    SpecDefaults::from_config(&FIXTURE_CONFIG, fixture_roots())
        .expect("fixture defaults should build")
}

pub fn minimal_entry() -> AppEntry {
    AppEntry {
        name: APP_NAME.to_string(),
        script: SCRIPT.to_string(),
        cwd: Some(CWD.to_string()),
        args: Vec::new(),
        rejected_env: None,
        depends_on: Vec::new(),
        autorestart: None,
        min_uptime_ms: None,
        max_restarts: None,
        restart_delay_ms: None,
        max_restart_delay_ms: None,
        listen_timeout_ms: None,
        ready_probe: None,
        liveness_tcp: None,
        schedule: None,
        max_memory: None,
        stop_exit_codes: Vec::new(),
        sandbox: None,
    }
}

pub fn sandbox_entry() -> SandboxEntry {
    SandboxEntry {
        mode: None,
        read: None,
        network: None,
        writable_roots: None,
        readable_roots: None,
    }
}

pub fn second_app_section(name: &str) -> String {
    apps_section(name, SCRIPT, CWD)
        .trim_start_matches("apps:\n")
        .to_string()
}

pub fn resolve_one(defaults: &SpecDefaults<'_>, entry: &AppEntry) -> AppSpec {
    resolve_checked(defaults, entry).expect("should resolve a single app")
}

pub fn resolve_one_err(defaults: &SpecDefaults<'_>, entry: &AppEntry) -> String {
    resolve_checked(defaults, entry).unwrap_err().to_string()
}

pub fn minimal_yaml() -> String {
    apps_section(APP_NAME, SCRIPT, CWD)
}

pub fn full_yaml() -> String {
    format!("{}{}", minimal_yaml(), every_optional_field_section())
}

pub fn write_apps_file(yaml: &str) -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("apps.yaml");
    std::fs::write(&path, yaml).expect("write apps file");
    let text = path.to_str().expect("path").to_string();
    (dir, text)
}
