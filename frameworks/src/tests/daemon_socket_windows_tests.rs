use std::time::Duration;

use tokio::net::windows::named_pipe::ClientOptions;

use super::*;

const ACCEPT_BUDGET: Duration = Duration::from_millis(200);
const ACCEPT_RETRY_MS: u64 = 5;
const EVERYONE: &str = "*S-1-1-0";

struct Socket {
    dir: tempfile::TempDir,
    path: std::path::PathBuf,
}

fn socket() -> Socket {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("pm3.sock");
    Socket { dir, path }
}

async fn pipe_of(path: &Path) -> String {
    let secret = crate::layout::pipe_secret(path)
        .await
        .expect("read the pipe secret");
    pipe_name_of(path, &secret)
}

async fn bound(path: &Path) -> PipeListener {
    match bind_uds(path, ACCEPT_RETRY_MS).await.expect("should bind") {
        BindOutcome::Bound(listener) => listener,
        BindOutcome::AlreadyRunning => panic!("the pipe should be free"),
    }
}

fn icacls(dir: &Path, args: &[&str]) {
    let status = std::process::Command::new("icacls")
        .arg(dir)
        .args(args)
        .output()
        .expect("icacls should run")
        .status;
    assert!(status.success(), "icacls {args:?} should succeed");
}

#[tokio::test]
async fn a_live_pipe_means_another_daemon_owns_it() {
    let socket = socket();
    let _held = bound(&socket.path).await;
    let outcome = bind_uds(&socket.path, ACCEPT_RETRY_MS)
        .await
        .expect("should detect the owner");
    assert!(
        matches!(outcome, BindOutcome::AlreadyRunning),
        "got: {outcome:?}"
    );
}

#[tokio::test]
async fn a_pipe_whose_every_instance_is_busy_is_reported() {
    let socket = socket();
    let name = pipe_of(&socket.path).await;
    let _server = ServerOptions::new()
        .first_pipe_instance(true)
        .max_instances(1)
        .create(&name)
        .expect("create the only instance");
    let _client = ClientOptions::new().open(&name).expect("occupy it");
    let err = bind_uds(&socket.path, ACCEPT_RETRY_MS)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot bind the pm3 socket"), "got: {err}");
}

#[tokio::test]
async fn a_socket_marker_blocked_by_a_directory_still_binds() {
    let socket = socket();
    std::fs::create_dir(&socket.path).expect("occupy the marker path");
    let _listener = bound(&socket.path).await;
    assert!(socket.path.is_dir(), "the blocker stays where it was");
}

#[tokio::test]
async fn a_socket_marker_that_cannot_be_written_still_binds() {
    let socket = socket();
    pipe_of(&socket.path).await;
    icacls(socket.dir.path(), &["/deny", &format!("{EVERYONE}:(WD)")]);
    let outcome = bind_uds(&socket.path, ACCEPT_RETRY_MS).await;
    icacls(socket.dir.path(), &["/remove:d", EVERYONE]);
    assert!(
        matches!(outcome, Ok(BindOutcome::Bound(_))),
        "got: {outcome:?}"
    );
    assert!(!socket.path.exists(), "the marker could not be written");
}

#[tokio::test]
async fn a_listener_serves_a_client_then_opens_the_next_instance() {
    let socket = socket();
    let mut listener = bound(&socket.path).await;
    let name = pipe_of(&socket.path).await;
    let _client = ClientOptions::new().open(&name).expect("connect");
    let served = tokio::time::timeout(ACCEPT_BUDGET, listener.accept()).await;
    assert!(served.is_ok(), "the daemon must serve its own client");
    assert!(
        listener.local_addr().is_ok(),
        "a pipe always has an address"
    );
    let idle = tokio::time::timeout(ACCEPT_BUDGET, listener.accept()).await;
    assert!(idle.is_err(), "no second client has connected");
}

#[tokio::test]
async fn a_pipe_instance_that_cannot_be_created_is_retried() {
    let socket = socket();
    let name = pipe_of(&socket.path).await;
    let first = ServerOptions::new()
        .first_pipe_instance(true)
        .create(&name)
        .expect("create the first instance");
    let invalid = format!(r"\\.\pipe\{}", "x".repeat(300));
    let mut listener = PipeListener::new(invalid, first, ACCEPT_RETRY_MS);
    let _client = ClientOptions::new().open(&name).expect("connect");
    let served = tokio::time::timeout(ACCEPT_BUDGET, listener.accept()).await;
    assert!(served.is_ok(), "the pending instance still serves");
    let retried = tokio::time::timeout(ACCEPT_BUDGET, listener.accept()).await;
    assert!(retried.is_err(), "an uncreatable instance keeps retrying");
}
