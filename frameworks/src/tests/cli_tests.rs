use super::*;
#[cfg(has_http)]
use crate::test_helpers::{
    serve_immediate_shutdown_retrying_bind, server_and_health_check_yaml, server_only_yaml,
    server_without_health_check_yaml,
};
use crate::test_helpers::{telemetry_only_yaml, tokio_block_on, write_config};

#[cfg(any(has_http, has_database))]
#[test]
fn require_section_present_returns_value() {
    let v = require_section(Some(42_i32), "server", "/x.yaml").expect("present");
    assert_eq!(v, 42);
}

#[cfg(any(has_http, has_database))]
#[test]
fn require_section_missing_returns_error() {
    let err = require_section::<i32>(None, "database", "/x.yaml").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("database"), "got: {msg}");
    assert!(msg.contains("/x.yaml"), "got: {msg}");
}

#[test]
fn run_config_check_ok() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_config(&dir, &telemetry_only_yaml());
    run_config_check(&path).expect("ok");
}

#[test]
fn run_config_check_invalid_returns_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_config(&dir, "not: valid: yaml: structure");
    assert!(run_config_check(&path).is_err());
}

#[test]
fn run_config_show_ok() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_config(&dir, &telemetry_only_yaml());
    run_config_show(&path).expect("ok");
}

#[test]
fn run_config_show_missing_file_returns_error() {
    assert!(run_config_show("/nonexistent/path/config.yaml").is_err());
}

#[cfg(has_http)]
#[tokio::test]
async fn run_serve_dry_run_ok() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_config(&dir, &server_only_yaml("127.0.0.1", 38900));
    run_serve(&path, true)
        .await
        .expect("dry-run should succeed");
}

#[cfg(has_http)]
#[tokio::test]
async fn run_serve_no_server_section_returns_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_config(&dir, &telemetry_only_yaml());
    let err = run_serve(&path, false).await.unwrap_err();
    assert!(err.to_string().contains("server"), "got: {err}");
}

#[cfg(has_http)]
#[tokio::test]
async fn run_serve_dry_run_no_server_section_returns_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_config(&dir, &telemetry_only_yaml());
    let err = run_serve(&path, true).await.unwrap_err();
    assert!(err.to_string().contains("server"), "got: {err}");
}

#[cfg(has_http)]
#[tokio::test]
async fn run_serve_invalid_config_returns_error() {
    assert!(run_serve("/nonexistent/path/x.yaml", true).await.is_err());
}

#[cfg(has_http)]
#[tokio::test]
async fn run_serve_with_shutdown_immediate_no_db() {
    let dir = tempfile::tempdir().expect("tempdir");
    serve_immediate_shutdown_retrying_bind(&dir, |port| server_only_yaml("127.0.0.1", port))
        .await
        .expect("immediate shutdown ok");
}

#[cfg(has_http)]
#[tokio::test]
async fn serve_retries_on_a_fresh_port_when_the_first_one_is_taken() {
    let occupied = std::net::TcpListener::bind("127.0.0.1:0").expect("occupy a port");
    let taken_port = occupied.local_addr().expect("local addr").port();
    let dir = tempfile::tempdir().expect("tempdir");
    let first_attempt = std::cell::Cell::new(true);

    serve_immediate_shutdown_retrying_bind(&dir, |fresh_port| {
        let port = if first_attempt.replace(false) {
            taken_port
        } else {
            fresh_port
        };
        server_only_yaml("127.0.0.1", port)
    })
    .await
    .expect("bind conflict on the first port must be retried, not surfaced");

    assert!(
        !first_attempt.get(),
        "the occupied port should have been attempted first"
    );
}

#[cfg(has_http)]
#[tokio::test]
async fn run_serve_dry_run_via_with_shutdown() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_config(&dir, &server_only_yaml("127.0.0.1", 38911));
    run_serve_with_shutdown(&path, true, async {})
        .await
        .expect("dry-run via with_shutdown ok");
}

#[cfg(has_http)]
#[tokio::test]
async fn run_serve_with_shutdown_bind_failure_returns_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let yaml = server_and_health_check_yaml("this-is-not.a.valid.host.ever-", 1, "127.0.0.1");
    let path = write_config(&dir, &yaml);
    assert!(
        run_serve_with_shutdown(&path, false, async {})
            .await
            .is_err()
    );
}

#[cfg(has_http)]
#[tokio::test]
async fn run_health_check_connect_ok() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let port = listener.local_addr().expect("local addr").port();
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_config(&dir, &server_only_yaml("127.0.0.1", port));
    run_health_check(&path).expect("connect should succeed");
    drop(listener);
}

#[cfg(has_http)]
#[test]
fn run_health_check_connect_fail() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_config(&dir, &server_only_yaml("127.0.0.1", 1));
    let err = run_health_check(&path).unwrap_err();
    assert!(
        err.to_string().contains("health check failed"),
        "got: {err}"
    );
}

#[cfg(has_http)]
#[test]
fn run_health_check_no_server_section_returns_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_config(&dir, &telemetry_only_yaml());
    assert!(run_health_check(&path).is_err());
}

#[cfg(has_http)]
#[test]
fn run_health_check_no_health_check_section_returns_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_config(&dir, &server_without_health_check_yaml("127.0.0.1", 9229));
    let err = run_health_check(&path).unwrap_err();
    assert!(err.to_string().contains("health_check"), "got: {err}");
}

#[cfg(has_http)]
#[test]
fn run_health_check_invalid_config_returns_error() {
    assert!(run_health_check("/nonexistent/health.yaml").is_err());
}

#[cfg(has_http)]
#[test]
fn health_check_target_wraps_bare_ipv6_literal() {
    assert_eq!(health_check_target("::1", 9229), "[::1]:9229");
}

#[cfg(has_http)]
#[test]
fn health_check_target_keeps_already_bracketed_ipv6() {
    assert_eq!(health_check_target("[::1]", 9229), "[::1]:9229");
}

#[cfg(has_http)]
#[test]
fn health_check_target_keeps_plain_host() {
    assert_eq!(health_check_target("127.0.0.1", 9229), "127.0.0.1:9229");
}

#[cfg(has_http)]
#[test]
fn per_address_timeout_splits_budget_across_addresses() {
    assert_eq!(
        per_address_timeout(Duration::from_secs(2), 2),
        Duration::from_secs(1)
    );
}

#[cfg(has_http)]
#[test]
fn per_address_timeout_keeps_whole_budget_for_single_address() {
    assert_eq!(
        per_address_timeout(Duration::from_secs(2), 1),
        Duration::from_secs(2)
    );
}

#[cfg(has_http)]
#[test]
fn per_address_timeout_never_divides_by_zero() {
    assert_eq!(
        per_address_timeout(Duration::from_secs(2), 0),
        Duration::from_secs(2)
    );
}

#[cfg(has_http)]
#[test]
fn health_check_failure_without_attempts_names_the_address() {
    let err = health_check_failure("localhost:9229", None);
    assert!(
        err.to_string().contains("resolved to no socket address"),
        "got: {err}"
    );
}

#[cfg(has_http)]
#[test]
fn run_health_check_invalid_host_address_maps_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let yaml = server_and_health_check_yaml("127.0.0.1", 9229, "this-is-not.a.valid.host.ever-");
    let path = write_config(&dir, &yaml);
    let err = run_health_check(&path).unwrap_err();
    assert!(
        err.to_string().contains("invalid health check address"),
        "got: {err}"
    );
}

#[cfg(has_http)]
#[test]
fn run_health_check_ipv6_already_bracketed_host_passes_through() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_config(&dir, &server_and_health_check_yaml("127.0.0.1", 1, "[::1]"));
    let err = run_health_check(&path).unwrap_err();
    assert!(
        err.to_string().contains("health check failed"),
        "got: {err}"
    );
}

#[cfg(has_http)]
#[test]
fn run_health_check_ipv6_literal_host_wraps_brackets() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_config(&dir, &server_and_health_check_yaml("127.0.0.1", 1, "::1"));
    let err = run_health_check(&path).unwrap_err();
    assert!(
        err.to_string().contains("health check failed"),
        "got: {err}"
    );
}

#[test]
fn dispatch_config_check() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_config(&dir, &telemetry_only_yaml());
    let cli = Cli {
        config: path,
        command: Commands::Config {
            command: ConfigCommands::Check,
        },
    };
    tokio_block_on(dispatch(cli)).expect("ok");
}

#[test]
fn dispatch_config_show() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_config(&dir, &telemetry_only_yaml());
    let cli = Cli {
        config: path,
        command: Commands::Config {
            command: ConfigCommands::Show,
        },
    };
    tokio_block_on(dispatch(cli)).expect("ok");
}

#[cfg(has_http)]
#[test]
fn dispatch_serve_dry_run() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_config(&dir, &server_only_yaml("127.0.0.1", 38905));
    let cli = Cli {
        config: path,
        command: Commands::Serve { dry_run: true },
    };
    tokio_block_on(dispatch(cli)).expect("ok");
}

#[cfg(has_http)]
#[test]
fn dispatch_health_check_fail() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_config(&dir, &server_only_yaml("127.0.0.1", 1));
    let cli = Cli {
        config: path,
        command: Commands::HealthCheck,
    };
    assert!(tokio_block_on(dispatch(cli)).is_err());
}
