#![allow(
    clippy::tests_outside_test_module,
    reason = "Cargo integration tests are inherently #[cfg(test)] scoped"
)]
#![cfg_attr(
    not(feature = "http"),
    allow(
        dead_code,
        unused_imports,
        reason = "sigint smoke test gated to feature=http; its helpers/imports are dead in default features"
    )
)]

use std::{
    path::Path,
    process::{Command, Stdio},
    time::Duration,
};

#[path = "../../adapters/test_support/config_sections.rs"]
mod config_sections;
#[path = "../../adapters/test_support/db_paths.rs"]
mod db_paths;
#[path = "../../adapters/test_support/net_ports.rs"]
mod net_ports;

use self::{
    config_sections::{database_section, health_check_section, server_section, telemetry_section},
    db_paths::{sqlite_rwc_url, workspace_migrations_dir},
    net_ports::ephemeral_port,
};

#[test]
fn binary_runs_config_check() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg_path = dir.path().join("config.yaml");
    std::fs::write(&cfg_path, telemetry_section("info")).expect("write config");

    let bin = env!("CARGO_BIN_EXE_skel_rs");
    let output = Command::new(bin)
        .args([
            "--config",
            cfg_path.to_str().expect("path"),
            "config",
            "check",
        ])
        .output()
        .expect("spawn skel_rs binary");

    assert!(
        output.status.success(),
        "binary exited non-zero: {:?}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Config OK"), "got stdout: {stdout}");
}

#[test]
fn binary_runs_config_show_with_env_placeholder() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg_path = dir.path().join("config.yaml");
    std::fs::write(
        &cfg_path,
        r#"
telemetry:
  service_name: "${SMOKE_SERVICE_NAME:-skel-smoke}"
  log_level: "info"
  log_format: "json"
"#,
    )
    .expect("write config");

    let bin = env!("CARGO_BIN_EXE_skel_rs");
    let output = Command::new(bin)
        .args([
            "--config",
            cfg_path.to_str().expect("path"),
            "config",
            "show",
        ])
        .output()
        .expect("spawn skel_rs binary");

    assert!(
        output.status.success(),
        "binary exited non-zero: {:?}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("skel-smoke"), "got stdout: {stdout}");
}

#[test]
fn binary_returns_error_on_missing_config() {
    let bin = env!("CARGO_BIN_EXE_skel_rs");
    let output = Command::new(bin)
        .args([
            "--config",
            "/nonexistent/path/config.yaml",
            "config",
            "check",
        ])
        .output()
        .expect("spawn skel_rs binary");

    assert!(!output.status.success(), "expected non-zero exit");
}

const UNBOUND_CONFIG_PORT: u16 = 9229;
const SERVER_START_ATTEMPTS: u8 = 3;
const LOOPBACK_HOST: &str = "127.0.0.1";
const SMOKE_DRAIN_TIMEOUT_SECS: u64 = 5;
const SMOKE_CONNECT_TIMEOUT_SECS: u64 = 2;
const SMOKE_MAX_CONNECTIONS: u32 = 5;
const FAST_DRAIN_TIMEOUT_SECS: u64 = 1;
const FAST_CONNECT_TIMEOUT_SECS: u64 = 1;

fn wait_until_listening(child: &mut std::process::Child, target: &str) -> bool {
    for _ in 0..40 {
        if std::net::TcpStream::connect(target).is_ok() {
            return true;
        }
        if child.try_wait().expect("try_wait").is_some() {
            return false;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

fn spawn_listening_server(
    dir: &Path,
    yaml_for_port: impl Fn(u16) -> String,
) -> (std::process::Child, String) {
    for _ in 0..SERVER_START_ATTEMPTS {
        let port = ephemeral_port();
        let cfg_path = dir.join("config.yaml");
        std::fs::write(&cfg_path, yaml_for_port(port)).expect("write config");
        let mut child = Command::new(env!("CARGO_BIN_EXE_skel_rs"))
            .args(["--config", cfg_path.to_str().expect("path"), "serve"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn skel_rs serve");
        let target = format!("127.0.0.1:{port}");
        if wait_until_listening(&mut child, &target) {
            return (child, target);
        }
        let _ = child.kill();
        let _ = child.wait();
    }
    panic!("server did not start after {SERVER_START_ATTEMPTS} attempts on fresh ephemeral ports");
}

fn write_smoke_config(cfg_path: &Path, port: u16, db_path: &Path, migrations: &Path) {
    std::fs::write(cfg_path, smoke_config_yaml(port, db_path, migrations)).expect("write config");
}

fn smoke_config_yaml(port: u16, db_path: &Path, migrations: &Path) -> String {
    format!(
        "{}{}{}{}",
        server_section(LOOPBACK_HOST, port, SMOKE_DRAIN_TIMEOUT_SECS),
        telemetry_section("info"),
        health_check_section(LOOPBACK_HOST, SMOKE_CONNECT_TIMEOUT_SECS),
        database_section(
            &sqlite_rwc_url(db_path),
            &migrations.display().to_string(),
            SMOKE_MAX_CONNECTIONS,
        ),
    )
}

#[cfg(feature = "http")]
#[test]
fn binary_serve_dry_run_exits_cleanly() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("dry.db");
    let cfg_path = dir.path().join("config.yaml");
    write_smoke_config(
        &cfg_path,
        UNBOUND_CONFIG_PORT,
        &db_path,
        &workspace_migrations_dir(),
    );

    let bin = env!("CARGO_BIN_EXE_skel_rs");
    let output = Command::new(bin)
        .args([
            "--config",
            cfg_path.to_str().expect("path"),
            "serve",
            "--dry-run",
        ])
        .output()
        .expect("spawn skel_rs serve --dry-run");

    assert!(
        output.status.success(),
        "dry-run should exit zero: {:?}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr),
    );
}

#[cfg(feature = "sqlite")]
#[test]
fn binary_db_migrate_then_status() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("smoke-db.db");
    let cfg_path = dir.path().join("config.yaml");
    write_smoke_config(
        &cfg_path,
        UNBOUND_CONFIG_PORT,
        &db_path,
        &workspace_migrations_dir(),
    );

    let bin = env!("CARGO_BIN_EXE_skel_rs");
    for sub in ["migrate", "status"] {
        let output = Command::new(bin)
            .args(["--config", cfg_path.to_str().expect("path"), "db", sub])
            .output()
            .expect("spawn skel_rs db subcommand");
        assert!(
            output.status.success(),
            "db {sub} should exit zero: {:?}\nstderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr),
        );
    }
}

fn spawn_config_subcommand(yaml: &str, sub: &str) -> std::process::Output {
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg_path = dir.path().join("config.yaml");
    std::fs::write(&cfg_path, yaml).expect("write config");
    Command::new(env!("CARGO_BIN_EXE_skel_rs"))
        .args(["--config", cfg_path.to_str().expect("path"), "config", sub])
        .output()
        .expect("spawn config subcommand")
}

#[cfg(unix)]
#[test]
fn binary_config_check_non_unicode_env_fails() {
    use std::{ffi::OsStr, os::unix::ffi::OsStrExt as _};

    let dir = tempfile::tempdir().expect("tempdir");
    let cfg_path = dir.path().join("config.yaml");
    std::fs::write(
        &cfg_path,
        "telemetry:\n  service_name: \"${SMOKE_BAD_UTF8}\"\n  log_level: \"info\"\n  log_format: \"json\"\n",
    )
    .expect("write config");
    let output = Command::new(env!("CARGO_BIN_EXE_skel_rs"))
        .env("SMOKE_BAD_UTF8", OsStr::from_bytes(&[0xff, 0xfe]))
        .args([
            "--config",
            cfg_path.to_str().expect("path"),
            "config",
            "check",
        ])
        .output()
        .expect("spawn config check");
    assert!(!output.status.success(), "non-unicode env should fail");
}

#[test]
fn binary_config_check_invalid_log_level_fails() {
    let yaml =
        "telemetry:\n  service_name: \"skel_rs\"\n  log_level: \"bogus\"\n  log_format: \"json\"\n";
    let output = spawn_config_subcommand(yaml, "check");
    assert!(!output.status.success(), "bogus log_level should fail");
}

#[test]
fn binary_config_show_missing_file_fails() {
    let output = Command::new(env!("CARGO_BIN_EXE_skel_rs"))
        .args([
            "--config",
            "/nonexistent/path/config.yaml",
            "config",
            "show",
        ])
        .output()
        .expect("spawn config show");
    assert!(
        !output.status.success(),
        "show on missing config should fail"
    );
}

#[test]
fn binary_config_check_passes_through_unparseable_placeholder() {
    let yaml =
        "telemetry:\n  service_name: \"${A${B\"\n  log_level: \"info\"\n  log_format: \"json\"\n";
    let output = spawn_config_subcommand(yaml, "check");
    assert!(
        output.status.success(),
        "unparseable placeholder passes through literally: {}",
        String::from_utf8_lossy(&output.stderr),
    );
}

#[cfg(feature = "http")]
#[test]
fn binary_health_check_error_paths_exit_nonzero() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bin = env!("CARGO_BIN_EXE_skel_rs");
    let cases = [
        ("missing-server", telemetry_section("info")),
        (
            "invalid-host",
            smoke_health_yaml("this-is-not.a.valid.host.ever-"),
        ),
        ("ipv6-unreachable", smoke_health_yaml("::1")),
        ("ipv6-bracketed", smoke_health_yaml("[::1]")),
    ];
    for (name, yaml) in cases {
        let cfg_path = dir.path().join(format!("{name}.yaml"));
        std::fs::write(&cfg_path, yaml).expect("write config");
        let output = Command::new(bin)
            .args(["--config", cfg_path.to_str().expect("path"), "health-check"])
            .output()
            .expect("spawn skel_rs health-check");
        assert!(
            !output.status.success(),
            "health-check {name} should exit non-zero"
        );
    }
}

#[cfg(feature = "http")]
fn smoke_health_yaml(hc_host: &str) -> String {
    format!(
        "{}{}{}",
        server_section(LOOPBACK_HOST, ephemeral_port(), FAST_DRAIN_TIMEOUT_SECS),
        telemetry_section("info"),
        health_check_section(hc_host, FAST_CONNECT_TIMEOUT_SECS),
    )
}

#[cfg(feature = "sqlite")]
#[test]
fn binary_db_status_missing_database_section_fails() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg_path = dir.path().join("config.yaml");
    std::fs::write(&cfg_path, telemetry_section("info")).expect("write config");

    let bin = env!("CARGO_BIN_EXE_skel_rs");
    let output = Command::new(bin)
        .args(["--config", cfg_path.to_str().expect("path"), "db", "status"])
        .output()
        .expect("spawn skel_rs db status");
    assert!(!output.status.success(), "db status should exit non-zero");
}

#[cfg(all(unix, feature = "http"))]
#[test]
fn binary_serve_handles_sigint_graceful_shutdown() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("smoke.db");
    let cfg_path = dir.path().join("config.yaml");
    let migrations = workspace_migrations_dir();
    let (mut child, _target) = spawn_listening_server(dir.path(), |port| {
        smoke_config_yaml(port, &db_path, &migrations)
    });

    let hc = Command::new(env!("CARGO_BIN_EXE_skel_rs"))
        .args(["--config", cfg_path.to_str().expect("path"), "health-check"])
        .output()
        .expect("spawn skel_rs health-check");
    assert!(
        hc.status.success(),
        "health-check should succeed while server is up: {}",
        String::from_utf8_lossy(&hc.stderr),
    );

    let kill_status = Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status()
        .expect("kill -INT");
    assert!(kill_status.success(), "kill failed");

    let exit = child.wait().expect("wait");
    assert!(
        exit.success(),
        "binary should exit cleanly on SIGINT, got: {exit:?}"
    );
}

#[cfg(all(unix, feature = "http"))]
#[test]
fn binary_serve_handles_sigterm_graceful_shutdown() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("sigterm.db");
    let migrations = workspace_migrations_dir();
    let (child, _target) = spawn_listening_server(dir.path(), |port| {
        smoke_config_yaml(port, &db_path, &migrations)
    });

    let kill_status = Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status()
        .expect("kill -TERM");
    assert!(kill_status.success(), "kill failed");

    let output = child.wait_with_output().expect("wait_with_output");
    assert!(
        output.status.success(),
        "binary should drain and exit cleanly on SIGTERM, got: {:?}",
        output.status
    );
    let logs = String::from_utf8_lossy(&output.stdout);
    assert!(
        logs.contains("SIGTERM"),
        "shutdown log must name the received signal, got:\n{logs}"
    );
}

#[cfg(all(unix, feature = "http"))]
fn debug_level_yaml(port: u16) -> String {
    format!(
        "{}{}{}",
        server_section(LOOPBACK_HOST, port, FAST_DRAIN_TIMEOUT_SECS),
        telemetry_section("debug"),
        health_check_section(LOOPBACK_HOST, FAST_CONNECT_TIMEOUT_SECS),
    )
}

#[cfg(all(unix, feature = "http"))]
fn get_health_with_request_id(target: &str, request_id: &str) -> String {
    use std::io::{Read as _, Write as _};

    let mut stream = std::net::TcpStream::connect(target).expect("connect for GET /health");
    let request = format!(
        "GET /health HTTP/1.1\r\nHost: {target}\r\nx-request-id: {request_id}\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .expect("write GET /health");

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .expect("read /health response");
    response
}

#[cfg(all(unix, feature = "http"))]
#[test]
fn handler_log_inherits_request_id_from_middleware_span() {
    const REQUEST_ID: &str = "smoke-trace-2f7c";

    let dir = tempfile::tempdir().expect("tempdir");
    let (child, target) = spawn_listening_server(dir.path(), debug_level_yaml);

    let response = get_health_with_request_id(&target, REQUEST_ID);
    Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status()
        .expect("kill -INT");
    let output = child.wait_with_output().expect("wait_with_output");

    assert!(response.contains("200 OK"), "got response: {response}");

    let logs = String::from_utf8_lossy(&output.stdout);
    let liveness_line = logs
        .lines()
        .find(|line| line.contains("liveness probe"))
        .unwrap_or_else(|| panic!("no liveness probe log line in stdout:\n{logs}"));
    assert!(
        liveness_line.contains(REQUEST_ID),
        "handler log must inherit request_id from middleware span, got: {liveness_line}"
    );
}
