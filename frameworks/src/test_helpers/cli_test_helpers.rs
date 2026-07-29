#![allow(
    dead_code,
    reason = "helper fns selectively consumed by feature-gated test modules"
)]

#[path = "../../../adapters/test_support/config_sections.rs"]
mod config_sections;
#[path = "../../../adapters/test_support/db_paths.rs"]
mod db_paths;
#[path = "../../../adapters/test_support/net_ports.rs"]
mod net_ports;
#[cfg(has_http)]
#[path = "../../../adapters/test_support/response_body.rs"]
mod response_body;

use self::config_sections::{
    database_section, health_check_section, server_section, telemetry_section,
};
#[cfg(has_database)]
pub use self::db_paths::{sqlite_rwc_url, workspace_migrations_dir};
#[cfg(has_http)]
pub use self::net_ports::ephemeral_port;
#[cfg(has_http)]
pub use self::response_body::body_json;

const INFO_LEVEL: &str = "info";
const DRAIN_TIMEOUT_SECS: u64 = 20;
const CONNECT_TIMEOUT_SECS: u64 = 2;
const MAX_CONNECTIONS: u32 = 5;

pub fn write_config(dir: &tempfile::TempDir, body: &str) -> String {
    let path = dir.path().join("config.yaml");
    std::fs::write(&path, body).expect("write config");
    path.to_str().expect("valid path").to_string()
}

pub fn full_yaml(host: &str, port: u16, db_url: &str, migrations_path: &str) -> String {
    format!(
        "{}{}{}{}",
        server_section(host, port, DRAIN_TIMEOUT_SECS),
        telemetry_section(INFO_LEVEL),
        health_check_section(host, CONNECT_TIMEOUT_SECS),
        database_section(db_url, migrations_path, MAX_CONNECTIONS),
    )
}

pub fn telemetry_only_yaml() -> String {
    telemetry_section(INFO_LEVEL)
}

pub fn server_only_yaml(host: &str, port: u16) -> String {
    server_and_health_check_yaml(host, port, host)
}

pub fn server_and_health_check_yaml(
    server_host: &str,
    port: u16,
    health_check_host: &str,
) -> String {
    format!(
        "{}{}{}",
        server_section(server_host, port, DRAIN_TIMEOUT_SECS),
        telemetry_section(INFO_LEVEL),
        health_check_section(health_check_host, CONNECT_TIMEOUT_SECS),
    )
}

pub fn server_without_health_check_yaml(host: &str, port: u16) -> String {
    format!(
        "{}{}",
        server_section(host, port, DRAIN_TIMEOUT_SECS),
        telemetry_section(INFO_LEVEL),
    )
}

pub fn tokio_block_on<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("rt")
        .block_on(f)
}

#[cfg(has_http)]
pub async fn serve_immediate_shutdown_retrying_bind(
    dir: &tempfile::TempDir,
    yaml_for_port: impl Fn(u16) -> String,
) -> anyhow::Result<()> {
    const MAX_ATTEMPTS: u8 = 5;

    let mut outcome = Ok(());
    for _ in 0..MAX_ATTEMPTS {
        let path = write_config(dir, &yaml_for_port(ephemeral_port()));
        outcome = crate::cli::run_serve_with_shutdown(&path, false, async {}).await;
        if !is_bind_conflict(&outcome) {
            break;
        }
    }
    outcome
}

#[cfg(has_http)]
fn is_bind_conflict(outcome: &anyhow::Result<()>) -> bool {
    let Err(err) = outcome else { return false };
    err.to_string().contains("cannot bind")
}
