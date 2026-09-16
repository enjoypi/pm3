use std::path::Path;

use usecases::SandboxMode;

use crate::{
    SpecSource,
    config_sections::{pm3_section, telemetry_section},
    parse_config,
};

pub const SERVICE_SCRIPT: &str = "/bin/sh";
pub const HOST_HOME: &str = "/home/dev";
pub const SANDBOX_MODE: &str = SandboxMode::WorkspaceWrite.as_str();

const KILL_TIMEOUT_MS: u64 = 1600;

pub fn spec_source_in(root: &Path) -> SpecSource {
    let home_dir = root.to_string_lossy().into_owned();
    let cfg_dir = root.join("service");
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
        apps_dir: home_dir.clone(),
        state_dir: home_dir.clone(),
        runtime_dir: home_dir.clone(),
        data_dir: home_dir.clone(),
        home_dir,
        host_home: Some(HOST_HOME.to_string()),
        logs_dir,
        tmp_dir: None,
    }
}

pub fn register_service(source: &SpecSource, name: &str) {
    write_service_file(source, name, &service_yaml(name));
}

pub fn write_service_file(source: &SpecSource, name: &str, body: &str) {
    let path = source.service(name).expect("a safe service name");
    std::fs::write(path, body).expect("write the service file");
}

pub fn write_env_file(source: &SpecSource, name: &str, body: &str) {
    let path = crate::env_file_of(&source.cfg_dir, name).expect("a safe service name");
    std::fs::write(path, body).expect("write the environment file");
}

#[cfg(unix)]
pub fn write_enc_file(source: &SpecSource, name: &str, body: &str) {
    let path = crate::enc_file_of(&source.cfg_dir, name).expect("a safe service name");
    std::fs::write(path, body).expect("write the encrypted file");
}

#[cfg(unix)]
pub fn with_decryptor(source: &mut SpecSource, body: &str) {
    use std::os::unix::fs::PermissionsExt as _;

    let path = source.cfg_dir.join("fake-sops");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write the decryptor stub");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
        .expect("make the decryptor stub executable");
    source.config.sops_program = path.to_string_lossy().into_owned();
    source.config.sops_identity_file = "/home/dev/.ssh/age-cfg".to_string();
}

pub fn service_yaml(name: &str) -> String {
    format!("name: \"{name}\"\nscript: \"{SERVICE_SCRIPT}\"\n")
}
