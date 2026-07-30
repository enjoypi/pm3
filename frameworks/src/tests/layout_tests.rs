use super::*;
use crate::test_support::pm3_config_with_home;

#[test]
fn an_absolute_home_becomes_the_layout_root() {
    let paths = resolve_layout(&pm3_config_with_home("/srv/pm3"), None).expect("should resolve");
    assert_eq!(paths.socket, Path::new("/srv/pm3/pm3.sock"));
}

#[test]
fn a_tilde_home_is_expanded_against_the_environment() {
    let paths =
        resolve_layout(&pm3_config_with_home("~/.pm3"), Some("/home/dev")).expect("should resolve");
    assert_eq!(paths.dump_file, Path::new("/home/dev/.pm3/dump.yaml"));
}

#[test]
fn a_tilde_home_without_an_environment_is_rejected() {
    let err = resolve_layout(&pm3_config_with_home("~/.pm3"), None)
        .unwrap_err()
        .to_string();
    assert!(err.contains("no HOME in the environment"), "got: {err}");
}

#[test]
fn the_host_home_comes_from_the_environment() {
    assert_eq!(host_home(), std::env::var("HOME").ok());
}

#[tokio::test]
async fn preparing_the_layout_creates_the_log_directory() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = resolve_paths(&dir.path().join("home"));
    ensure_layout(&paths).await.expect("should prepare");
    assert!(paths.logs_dir.is_dir());
}

#[tokio::test]
async fn preparing_the_layout_reports_a_blocked_root() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path().join("home");
    std::fs::write(&root, "blocked").expect("occupy the root");
    let err = ensure_layout(&resolve_paths(&root))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot prepare the pm3 home"), "got: {err}");
}

#[tokio::test]
async fn the_pid_file_records_this_process() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = resolve_paths(dir.path());
    write_pid_file(&paths).await.expect("should write");
    let recorded = std::fs::read_to_string(&paths.pid_file).expect("read");
    assert_eq!(recorded, std::process::id().to_string());
}

#[tokio::test]
async fn a_blocked_pid_path_is_reported() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = resolve_paths(dir.path());
    std::fs::create_dir(&paths.pid_file).expect("occupy the pid path");
    let err = write_pid_file(&paths).await.unwrap_err().to_string();
    assert!(err.contains("cannot prepare the pm3 home"), "got: {err}");
}

#[tokio::test]
async fn clearing_runtime_files_removes_the_socket_and_the_pid_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = resolve_paths(dir.path());
    std::fs::write(&paths.socket, "socket").expect("seed socket");
    write_pid_file(&paths).await.expect("should write");
    clear_runtime_files(&paths).await;
    assert!(!paths.socket.exists() && !paths.pid_file.exists());
}

#[tokio::test]
async fn clearing_runtime_files_tolerates_missing_files() {
    let dir = tempfile::tempdir().expect("temp dir");
    clear_runtime_files(&resolve_paths(dir.path())).await;
    assert!(!dir.path().join("pm3.sock").exists());
}
