use std::path::Path;

use usecases::SandboxMode;

use crate::{
    SpecSource,
    config::app::parse_config,
    config_sections::{pm3_section, telemetry_section},
};

pub const SERVICE_SCRIPT: &str = crate::platform::SHELL;
pub const HOST_HOME: &str = "/home/dev";
pub const SANDBOX_MODE: &str = SandboxMode::WorkspaceWrite.as_str();

const KILL_TIMEOUT_MS: u64 = 1600;

pub fn spec_source_in(root: &Path) -> SpecSource {
    let home_dir = crate::portable_path(&root.to_string_lossy());
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
    let logs_dir = format!("{home_dir}/logs");
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
        global_env: Vec::new(),
        decryptor_env: Vec::new(),
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
    let path = crate::apps_file::env_file_of(&source.cfg_dir, name).expect("a safe service name");
    std::fs::write(path, body).expect("write the environment file");
}

pub fn write_enc_file(source: &SpecSource, name: &str, body: &str) {
    let path = crate::apps_file::enc_file_of(&source.cfg_dir, name).expect("a safe service name");
    std::fs::write(path, body).expect("write the encrypted file");
}

pub fn with_decryptor(source: &mut SpecSource, unix: &str, windows: &str) {
    let path = crate::platform::script(&source.cfg_dir, "fake-sops", unix, windows);
    source.config.sops_program = path.to_string_lossy().into_owned();
    source.config.sops_identity_file = "/home/dev/.ssh/age-cfg".to_string();
}

pub fn service_yaml(name: &str) -> String {
    format!("name: \"{name}\"\nscript: '{SERVICE_SCRIPT}'\n")
}

pub fn with_global_env(source: &mut SpecSource, declared: &[(&str, &str)]) {
    source.global_env = declared
        .iter()
        .map(|(key, value)| usecases::EnvValue::new(key, value, usecases::EnvScope::Global))
        .collect();
}
