use super::*;

fn temp_dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("temp dir")
}

#[tokio::test]
async fn a_free_path_is_bound() {
    let dir = temp_dir();
    let outcome = bind_uds(&dir.path().join("pm3.sock"))
        .await
        .expect("should bind");
    assert!(matches!(outcome, BindOutcome::Bound(_)), "got: {outcome:?}");
}

#[tokio::test]
async fn a_live_socket_means_another_daemon_owns_it() {
    let dir = temp_dir();
    let path = dir.path().join("pm3.sock");
    let _held = UnixListener::bind(&path).expect("bind the first daemon");
    let outcome = bind_uds(&path).await.expect("should detect the owner");
    assert!(
        matches!(outcome, BindOutcome::AlreadyRunning),
        "got: {outcome:?}"
    );
}

#[tokio::test]
async fn a_stale_socket_file_is_replaced() {
    let dir = temp_dir();
    let path = dir.path().join("pm3.sock");
    std::fs::write(&path, "orphan").expect("seed a stale socket file");
    let outcome = bind_uds(&path).await.expect("should self-heal");
    assert!(matches!(outcome, BindOutcome::Bound(_)), "got: {outcome:?}");
}

#[tokio::test]
async fn a_socket_path_blocked_by_a_directory_is_reported() {
    let dir = temp_dir();
    let path = dir.path().join("pm3.sock");
    std::fs::create_dir(&path).expect("occupy the socket path");
    let err = bind_uds(&path).await.unwrap_err().to_string();
    assert!(
        err.contains("cannot remove the stale pm3 socket"),
        "got: {err}"
    );
}

#[tokio::test]
async fn an_unbindable_socket_path_is_reported() {
    let dir = temp_dir();
    let path = dir.path().join("absent").join("pm3.sock");
    let err = bind_uds(&path).await.unwrap_err().to_string();
    assert!(err.contains("cannot bind the pm3 socket"), "got: {err}");
}
