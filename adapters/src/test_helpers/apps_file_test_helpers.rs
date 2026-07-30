use super::*;
use crate::{
    apps_sections::{apps_section, every_optional_field_section},
    config::{SANDBOX_MODE_WORKSPACE_WRITE, parse_config},
    config_sections::{pm3_section, telemetry_section},
};

pub const APP_NAME: &str = "web";
pub const SCRIPT: &str = "/usr/bin/node";
pub const CWD: &str = "/srv/web";
pub const LOGS_DIR: &str = "/tmp/pm3-fixture/logs";
pub const TMP_DIR: &str = "/tmp/pm3-fixture-tmp";

pub fn pm3_config(sandbox_mode: &str) -> Pm3Config {
    let yaml = format!(
        "{}{}",
        pm3_section("/tmp/pm3-fixture", 1600, sandbox_mode),
        telemetry_section("info"),
    );
    parse_config(&yaml)
        .expect("fixture config should parse")
        .pm3
}

pub fn defaults() -> SpecDefaults<'static> {
    SpecDefaults::from_config(
        &pm3_config(SANDBOX_MODE_WORKSPACE_WRITE),
        LOGS_DIR,
        Some(TMP_DIR),
    )
    .expect("fixture defaults should build")
}

pub fn minimal_entry() -> AppEntry {
    AppEntry {
        name: APP_NAME.to_string(),
        script: SCRIPT.to_string(),
        cwd: CWD.to_string(),
        args: Vec::new(),
        env: BTreeMap::new(),
        depends_on: Vec::new(),
        autorestart: None,
        min_uptime_ms: None,
        max_restarts: None,
        restart_delay_ms: None,
        sandbox: None,
    }
}

pub fn sandbox_entry() -> SandboxEntry {
    SandboxEntry {
        mode: None,
        network: None,
        writable_roots: None,
    }
}

pub fn apps_of(entries: Vec<AppEntry>) -> AppsFile {
    AppsFile { apps: entries }
}

pub fn resolve_one(defaults: &SpecDefaults<'_>, entry: AppEntry) -> AppSpec {
    let mut specs =
        resolve_specs(defaults, &apps_of(vec![entry])).expect("should resolve a single app");
    specs.pop().expect("should yield one spec")
}

pub fn resolve_one_err(defaults: &SpecDefaults<'_>, entry: AppEntry) -> String {
    resolve_specs(defaults, &apps_of(vec![entry]))
        .unwrap_err()
        .to_string()
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
