use std::{path::PathBuf, time::Duration};

use adapters::resolve_paths;
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::UnixListener,
};

use super::*;
use crate::test_support::pm3_config_with_home;

const REPLY_200: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok";
const REQUEST_SINK: usize = 1024;
const NEVER_BINDS: &str = "/usr/bin/true";

struct Fixture {
    dir: tempfile::TempDir,
    paths: Pm3Paths,
    config_path: String,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = resolve_paths(dir.path());
    let config_path = crate::test_support::write_config(dir.path(), &paths.root.to_string_lossy())
        .to_string_lossy()
        .into_owned();
    Fixture {
        dir,
        paths,
        config_path,
    }
}

fn launch<'f>(fixture: &'f Fixture, program: &str) -> DaemonLaunch<'f> {
    DaemonLaunch::from_config(
        &fixture.paths,
        &fixture.config_path,
        PathBuf::from(program),
        &pm3_config_with_home(&fixture.paths.root.to_string_lossy()),
    )
}

fn healthy_daemon(paths: &Pm3Paths) -> tokio::task::JoinHandle<()> {
    let listener = UnixListener::bind(&paths.socket).expect("bind");
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _addr)) = listener.accept().await else {
                return;
            };
            let mut sink = vec![0_u8; REQUEST_SINK];
            let read = stream.read(&mut sink).await.unwrap_or_default();
            sink.truncate(read);
            stream.write_all(REPLY_200).await.ok();
            stream.shutdown().await.ok();
        }
    })
}

#[test]
fn the_poll_budget_comes_from_the_configured_timeouts() {
    let fixture = fixture();
    let launch = launch(&fixture, NEVER_BINDS);
    assert_eq!(launch.interval_ms, crate::test_support::POLL_INTERVAL_MS);
    assert_eq!(
        launch.attempts,
        u32::try_from(
            crate::test_support::START_TIMEOUT_MS / crate::test_support::POLL_INTERVAL_MS
        )
        .expect("the fixture budget fits")
    );
}

#[tokio::test]
async fn a_daemon_that_already_answers_is_left_alone() {
    let fixture = fixture();
    let serving = healthy_daemon(&fixture.paths);
    ensure_daemon_running(&launch(&fixture, NEVER_BINDS))
        .await
        .expect("should find the running daemon");
    serving.abort();
    assert!(!fixture.paths.lock_file.exists(), "no lock should be taken");
}

#[tokio::test]
async fn an_unspawnable_daemon_program_is_reported() {
    let fixture = fixture();
    let err = ensure_daemon_running(&launch(&fixture, "/nonexistent/pm3"))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot spawn the pm3 daemon"), "got: {err}");
}

#[tokio::test]
async fn the_spawn_lock_is_released_after_a_failed_spawn() {
    let fixture = fixture();
    ensure_daemon_running(&launch(&fixture, "/nonexistent/pm3"))
        .await
        .expect_err("should fail");
    assert!(
        !fixture.paths.lock_file.exists(),
        "the lock must not outlive the attempt"
    );
}

#[tokio::test]
async fn an_unwritable_daemon_log_is_reported() {
    let fixture = fixture();
    std::fs::create_dir(&fixture.paths.daemon_log).expect("occupy the daemon log path");
    let err = ensure_daemon_running(&launch(&fixture, NEVER_BINDS))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot prepare the pm3 home"), "got: {err}");
}

#[tokio::test]
async fn a_daemon_that_never_answers_times_out() {
    let fixture = fixture();
    let err = ensure_daemon_running(&launch(&fixture, NEVER_BINDS))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot reach the pm3 daemon"), "got: {err}");
}

#[tokio::test]
async fn a_held_lock_makes_the_caller_wait_instead_of_spawning() {
    let fixture = fixture();
    std::fs::write(&fixture.paths.lock_file, "held").expect("hold the lock");
    let socket = fixture.paths.socket.clone();
    let serving = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(80)).await;
        let listener = UnixListener::bind(&socket).expect("bind");
        loop {
            let Ok((mut stream, _addr)) = listener.accept().await else {
                return;
            };
            let mut sink = vec![0_u8; REQUEST_SINK];
            let read = stream.read(&mut sink).await.unwrap_or_default();
            sink.truncate(read);
            stream.write_all(REPLY_200).await.ok();
            stream.shutdown().await.ok();
        }
    });

    ensure_daemon_running(&launch(&fixture, "/nonexistent/pm3"))
        .await
        .expect("should wait for the other starter");
    serving.abort();
    assert!(
        fixture.paths.lock_file.exists(),
        "the other starter still owns the lock"
    );
    drop(fixture.dir);
}
