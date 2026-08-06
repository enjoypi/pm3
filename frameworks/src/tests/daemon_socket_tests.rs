use std::time::Duration;

use super::*;

const ACCEPT_BUDGET: Duration = Duration::from_millis(200);

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
async fn a_bound_socket_is_owner_only() {
    let dir = temp_dir();
    let path = dir.path().join("pm3.sock");
    let outcome = bind_uds(&path).await.expect("should bind");
    assert!(matches!(outcome, BindOutcome::Bound(_)), "got: {outcome:?}");
    let mode = std::fs::metadata(&path)
        .expect("stat the socket")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600, "got: {mode:o}");
}

#[tokio::test]
async fn an_unrestrictable_socket_path_is_reported() {
    let dir = temp_dir();
    let err = restrict_to_owner(&dir.path().join("absent.sock"))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot restrict the pm3 socket"), "got: {err}");
}

#[tokio::test]
async fn an_unbindable_socket_path_is_reported() {
    let dir = temp_dir();
    let path = dir.path().join("absent").join("pm3.sock");
    let err = bind_uds(&path).await.unwrap_err().to_string();
    assert!(err.contains("cannot bind the pm3 socket"), "got: {err}");
}

#[test]
fn a_peer_running_as_the_owner_is_admitted() {
    assert!(admits(Some(501), Some(501)));
}

#[test]
fn a_peer_running_as_someone_else_is_turned_away() {
    assert!(!admits(Some(502), Some(501)));
}

#[test]
fn an_unreadable_credential_falls_back_to_the_socket_permissions() {
    assert!(
        admits(None, Some(501)),
        "the socket mode and the directory mode remain the standing defence"
    );
}

#[test]
fn an_unknown_owner_falls_back_to_the_socket_permissions() {
    assert!(admits(Some(502), None));
}

#[tokio::test]
async fn a_connection_from_the_owner_reaches_the_router() {
    let dir = temp_dir();
    let path = dir.path().join("pm3.sock");
    let mut listener = OwnerOnlyListener::new(
        UnixListener::bind(&path).expect("bind the socket"),
        owner_uid_of(&path),
    );
    let _client = UnixStream::connect(&path).await.expect("connect");
    let accepted = tokio::time::timeout(ACCEPT_BUDGET, listener.accept()).await;
    assert!(accepted.is_ok(), "the daemon must serve its own user");
}

#[tokio::test]
async fn a_connection_from_another_user_never_reaches_the_router() {
    let dir = temp_dir();
    let path = dir.path().join("pm3.sock");
    let mut listener = OwnerOnlyListener::new(
        UnixListener::bind(&path).expect("bind the socket"),
        Some(u32::MAX),
    );
    let _client = UnixStream::connect(&path).await.expect("connect");
    let accepted = tokio::time::timeout(ACCEPT_BUDGET, listener.accept()).await;
    assert!(
        accepted.is_err(),
        "a peer outside the owning user must be dropped before it can send a request"
    );
}

#[tokio::test]
async fn a_bound_listener_reports_the_address_it_answers_on() {
    let dir = temp_dir();
    let path = dir.path().join("pm3.sock");
    let listener = OwnerOnlyListener::new(
        UnixListener::bind(&path).expect("bind the socket"),
        owner_uid_of(&path),
    );
    let reported = listener
        .local_addr()
        .expect("a bound socket knows its path");
    assert_eq!(reported.as_pathname(), Some(path.as_path()));
}

#[test]
fn a_socket_whose_owner_cannot_be_read_leaves_the_owner_unknown() {
    assert_eq!(socket_owner_of(Path::new("/nonexistent/pm3.sock")), None);
}
