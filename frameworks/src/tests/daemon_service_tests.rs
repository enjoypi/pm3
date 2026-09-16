#![cfg(unix)]
use std::time::Duration;

use adapters::{Pm3Roots, resolve_paths};

use super::*;
use crate::{
    client::UdsClient,
    signal::SignalRegisterError,
    test_support::{REQUEST_TIMEOUT_MS, write_config},
};

const READY_BUDGET: Duration = Duration::from_secs(5);
const PROBE_INTERVAL: Duration = Duration::from_millis(20);

async fn wait_until_healthy(client: &UdsClient) {
    let deadline = tokio::time::Instant::now() + READY_BUDGET;
    while tokio::time::Instant::now() < deadline {
        if client.daemon_is_healthy().await {
            return;
        }
        tokio::time::sleep(PROBE_INTERVAL).await;
    }
    panic!("the daemon should answer inside the budget")
}

#[tokio::test]
async fn a_daemon_whose_signal_handlers_cannot_register_refuses_to_start() {
    let failed: std::result::Result<ShutdownSignals, SignalRegisterError> =
        Err(SignalRegisterError::Register {
            signal: "SIGTERM",
            reason: "too many open files".to_string(),
        });
    let err = run_daemon_with_signals("/nonexistent/pm3.yaml", failed)
        .await
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("cannot register the SIGTERM handler"),
        "got: {err}"
    );
}

#[tokio::test]
async fn a_daemon_with_registered_handlers_still_needs_a_config() {
    let signals = ShutdownSignals::register().expect("register the shutdown handlers");
    let outcome = run_daemon_with_signals("/nonexistent/pm3.yaml", Ok(signals)).await;
    assert!(outcome.is_err(), "got: {outcome:?}");
}

#[tokio::test]
async fn a_missing_config_stops_the_daemon() {
    let outcome = run_daemon_with_shutdown("/nonexistent/pm3.yaml", Box::pin(async {})).await;
    assert!(outcome.is_err(), "got: {outcome:?}");
}

#[tokio::test]
async fn a_blocked_home_stops_the_daemon() {
    let dir = tempfile::tempdir().expect("temp dir");
    let home = dir.path().join("home");
    std::fs::write(&home, "blocked").expect("occupy the home");
    let config = write_config(dir.path(), &home.to_string_lossy());
    let err = run_daemon_with_shutdown(config.to_str().expect("path"), Box::pin(async {}))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot prepare the pm3 home"), "got: {err}");
}

#[tokio::test]
async fn a_socket_path_blocked_by_a_directory_stops_the_daemon() {
    let dir = tempfile::tempdir().expect("temp dir");
    let home = dir.path().join("home");
    let paths = resolve_paths(Pm3Roots::single(&home));
    std::fs::create_dir_all(&paths.logs_dir).expect("prepare the home");
    std::fs::create_dir(&paths.socket).expect("occupy the socket path");
    let config = write_config(dir.path(), &home.to_string_lossy());
    let err = run_daemon_with_shutdown(config.to_str().expect("path"), Box::pin(async {}))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("stale pm3 socket"), "got: {err}");
}

#[tokio::test]
async fn a_daemon_that_cannot_decrypt_an_environment_refuses_to_take_over() {
    use std::os::unix::fs::PermissionsExt as _;

    let dir = tempfile::tempdir().expect("temp dir");
    let home = dir.path().join("home");
    let paths = resolve_paths(Pm3Roots::single(&home));
    let cfg_dir = home.join("service");
    std::fs::create_dir_all(&paths.logs_dir).expect("prepare the home");
    std::fs::create_dir_all(&cfg_dir).expect("prepare the service directory");
    std::fs::write(
        cfg_dir.join("web.yaml"),
        "name: \"web\"\nscript: \"/bin/sh\"\n",
    )
    .expect("write the service file");
    std::fs::write(cfg_dir.join("web.enc.yaml"), "TOKEN: ENC[fake]\n")
        .expect("write the encrypted environment");
    let decryptor = dir.path().join("fake-sops");
    std::fs::write(&decryptor, "#!/bin/sh\nexit 4\n").expect("write the decryptor stub");
    std::fs::set_permissions(&decryptor, std::fs::Permissions::from_mode(0o755))
        .expect("make the decryptor stub executable");
    std::fs::write(
        &paths.dump_file,
        "services:\n  - name: web\n    runtime:\n      pm_id: 0\n      status: stopped\n      restart_time: 0\n      unstable_restarts: 0\n      created_at_ms: 1\n",
    )
    .expect("seed the dump");
    let config = crate::test_support::write_config_with_decryptor(
        dir.path(),
        &home.to_string_lossy(),
        &decryptor.to_string_lossy(),
    );

    let err = run_daemon_with_shutdown(config.to_str().expect("path"), Box::pin(async {}))
        .await
        .unwrap_err()
        .to_string();

    assert!(err.contains("cannot take over"), "got: {err}");
    assert!(
        !paths.socket.exists(),
        "a daemon that stood down must leave no socket behind"
    );
}

#[tokio::test]
async fn a_second_daemon_on_a_live_socket_exits_quietly() {
    let dir = tempfile::tempdir().expect("temp dir");
    let home = dir.path().join("home");
    let paths = resolve_paths(Pm3Roots::single(&home));
    std::fs::create_dir_all(&paths.logs_dir).expect("prepare the home");
    let _held = tokio::net::UnixListener::bind(&paths.socket).expect("bind the first daemon");
    let config = write_config(dir.path(), &home.to_string_lossy());
    run_daemon_with_shutdown(config.to_str().expect("path"), Box::pin(async {}))
        .await
        .expect("the second daemon should stand down");
}

#[tokio::test]
async fn a_blocked_pid_path_stops_the_daemon() {
    let dir = tempfile::tempdir().expect("temp dir");
    let home = dir.path().join("home");
    let paths = resolve_paths(Pm3Roots::single(&home));
    std::fs::create_dir_all(&paths.logs_dir).expect("prepare the home");
    std::fs::create_dir(&paths.pid_file).expect("occupy the pid path");
    let config = write_config(dir.path(), &home.to_string_lossy());
    let err = run_daemon_with_shutdown(config.to_str().expect("path"), Box::pin(async {}))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot prepare the pm3 home"), "got: {err}");
}

#[tokio::test]
async fn a_running_daemon_serves_and_cleans_up_after_itself() {
    let dir = tempfile::tempdir().expect("temp dir");
    let home = dir.path().join("home");
    let paths = resolve_paths(Pm3Roots::single(&home));
    let config = write_config(dir.path(), &home.to_string_lossy());
    let config_path = config.to_str().expect("path").to_string();
    let (shutdown, wait) = tokio::sync::oneshot::channel::<()>();

    let daemon = tokio::spawn(async move {
        run_daemon_with_shutdown(
            &config_path,
            Box::pin(async move {
                wait.await.ok();
            }),
        )
        .await
    });

    let client = UdsClient::new(paths.socket.clone(), REQUEST_TIMEOUT_MS);
    wait_until_healthy(&client).await;
    assert!(paths.pid_file.is_file(), "the pid file should be written");

    shutdown.send(()).expect("signal shutdown");
    daemon.await.expect("join").expect("serve ok");
    assert!(
        !paths.socket.exists() && !paths.pid_file.exists(),
        "the daemon should clean up its runtime files"
    );
}

#[tokio::test]
async fn a_running_daemon_answers_a_list_request() {
    let dir = tempfile::tempdir().expect("temp dir");
    let home = dir.path().join("home");
    let paths = resolve_paths(Pm3Roots::single(&home));
    let config = write_config(dir.path(), &home.to_string_lossy());
    let config_path = config.to_str().expect("path").to_string();
    let (shutdown, wait) = tokio::sync::oneshot::channel::<()>();

    let daemon = tokio::spawn(async move {
        run_daemon_with_shutdown(
            &config_path,
            Box::pin(async move {
                wait.await.ok();
            }),
        )
        .await
    });

    let client = UdsClient::new(paths.socket.clone(), REQUEST_TIMEOUT_MS);
    wait_until_healthy(&client).await;
    let reply = client
        .request("GET", "/apps", None)
        .await
        .expect("should answer");
    assert!(reply.body.contains("no apps"), "got: {}", reply.body);

    shutdown.send(()).expect("signal shutdown");
    daemon.await.expect("join").expect("serve ok");
}

#[tokio::test]
async fn a_relative_home_stops_the_daemon() {
    let dir = tempfile::tempdir().expect("temp dir");
    let config = write_config(dir.path(), "relative/home");
    let err = run_daemon_with_shutdown(config.to_str().expect("path"), Box::pin(async {}))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("must be absolute"), "got: {err}");
}

#[tokio::test]
async fn a_relative_service_directory_stops_the_daemon() {
    let dir = tempfile::tempdir().expect("temp dir");
    let home = dir.path().join("home");
    let config = crate::test_support::write_config_with_cfg_dir(
        dir.path(),
        &home.to_string_lossy(),
        "relative/service",
    );
    let err = run_daemon_with_shutdown(config.to_str().expect("path"), Box::pin(async {}))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("must be absolute"), "got: {err}");
}

#[tokio::test]
async fn a_daemon_refuses_a_home_that_overflows_the_socket_limit() {
    let dir = tempfile::tempdir().expect("temp dir");
    let deep = dir.path().join("d".repeat(120));
    let config_path = write_config(dir.path(), &deep.to_string_lossy());
    let signals = ShutdownSignals::register().expect("register the shutdown handlers");

    let err = run_daemon_with_signals(&config_path.to_string_lossy(), Ok(signals))
        .await
        .unwrap_err()
        .to_string();

    assert!(err.contains("exceeds the 104"), "got: {err}");
}
