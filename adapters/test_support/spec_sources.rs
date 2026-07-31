use std::path::Path;

use crate::{
    SpecSource,
    config_sections::{pm3_section, telemetry_section},
    parse_config,
};

pub const SERVICE_SCRIPT: &str = "/bin/sh";
pub const SANDBOX_MODE: &str = "workspace-write";

const KILL_TIMEOUT_MS: u64 = 1600;

pub fn spec_source_in(root: &Path) -> SpecSource {
    let home_dir = root.to_string_lossy().into_owned();
    let cfg_dir = root.join("svc");
    std::fs::create_dir_all(&cfg_dir).expect("create the service directory");
    let yaml = format!(
        "{}{}",
        pm3_section(&home_dir, KILL_TIMEOUT_MS, SANDBOX_MODE),
        telemetry_section("info"),
    );
    let config = parse_config(&yaml)
        .expect("the fixture config should parse")
        .pm3;
    let logs_dir = root.join("logs").to_string_lossy().into_owned();
    SpecSource {
        cfg_dir,
        config,
        home_dir,
        logs_dir,
        tmp_dir: None,
    }
}

pub fn register_service(source: &SpecSource, name: &str) {
    write_service_file(source, name, &service_yaml(name));
}

pub fn write_service_file(source: &SpecSource, name: &str, body: &str) {
    std::fs::write(source.service_file(name), body).expect("write the service file");
}

pub fn service_yaml(name: &str) -> String {
    format!("name: \"{name}\"\nscript: \"{SERVICE_SCRIPT}\"\n")
}
