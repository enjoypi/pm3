use super::*;

#[test]
fn a_pipe_name_changes_with_the_secret() {
    let socket = Path::new(r"C:\pm3\pm3.sock");
    let first = pipe_name_of(socket, "0123456789abcdef0123456789abcdef");
    let second = pipe_name_of(socket, "fedcba9876543210fedcba9876543210");
    assert_ne!(first, second);
    assert!(first.starts_with(r"\\.\pipe\pm3-"), "got: {first}");
}

#[test]
fn a_generated_secret_has_the_shape_of_a_pipe_secret() {
    let secret = generate_pipe_secret();
    assert!(is_pipe_secret(&secret), "got: {secret}");
}

#[test]
fn a_secret_with_a_bad_shape_is_rejected() {
    assert!(!is_pipe_secret("too-short"));
    assert!(!is_pipe_secret("0123456789abcdef0123456789abcdeg"));
    assert!(!is_pipe_secret("0123456789ABCDEF0123456789ABCDEF0"));
}

#[tokio::test]
async fn a_missing_pipe_secret_is_created_once_and_reused() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("pm3.sock");
    let first = pipe_secret(&socket).await.expect("create the secret");
    let second = pipe_secret(&socket).await.expect("read the secret back");
    assert_eq!(first, second);
    assert!(is_pipe_secret(&first), "got: {first}");
}

#[tokio::test]
async fn a_corrupt_pipe_secret_is_replaced() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("pm3.sock");
    tokio::fs::write(socket.with_file_name(PIPE_SECRET_FILE), "garbage")
        .await
        .expect("seed a corrupt secret");
    let secret = pipe_secret(&socket).await.expect("replace the secret");
    assert!(is_pipe_secret(&secret), "got: {secret}");
}

#[tokio::test]
async fn a_pipe_secret_in_a_missing_home_is_reported() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("absent").join("pm3.sock");
    let err = pipe_secret(&socket).await.unwrap_err().to_string();
    assert!(err.contains("absent"), "got: {err}");
}

#[tokio::test]
async fn a_pipe_secret_blocked_by_a_directory_is_reported() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("pm3.sock");
    std::fs::create_dir(socket.with_file_name(PIPE_SECRET_FILE)).expect("block the secret path");
    let err = pipe_secret(&socket).await.unwrap_err().to_string();
    assert!(err.contains(PIPE_SECRET_FILE), "got: {err}");
}

#[tokio::test]
async fn a_lost_create_race_reads_the_winners_secret() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join(PIPE_SECRET_FILE);
    let winner = "0123456789abcdef0123456789abcdef";
    tokio::fs::write(&path, winner)
        .await
        .expect("seed the winner");
    let secret = create_pipe_secret(&path).await.expect("read the winner");
    assert_eq!(secret, winner);
}

const SHARE_READ_ONLY: u32 = 0x1;

#[tokio::test]
async fn a_corrupt_pipe_secret_that_cannot_be_removed_is_reported() {
    use std::os::windows::fs::OpenOptionsExt as _;

    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("pm3.sock");
    let path = socket.with_file_name(PIPE_SECRET_FILE);
    std::fs::write(&path, "garbage").expect("seed a corrupt secret");
    let _pinned = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(SHARE_READ_ONLY)
        .open(&path)
        .expect("hold the corrupt secret without delete sharing");

    let err = pipe_secret(&socket).await.unwrap_err().to_string();

    assert!(err.contains(PIPE_SECRET_FILE), "got: {err}");
}
