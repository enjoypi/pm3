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

#[test]
fn a_readable_path_reports_the_uid_that_owns_it() {
    let dir = tempfile::tempdir().expect("temp dir");
    let owner = std::fs::metadata(dir.path()).expect("stat the temp dir");
    assert_eq!(owner_uid_of(dir.path()), Some(owner.uid()));
}

#[test]
fn a_path_this_platform_does_not_offer_reports_no_owner() {
    assert_eq!(owner_uid_of(Path::new("/nonexistent/pm3-owner")), None);
}

#[test]
fn the_host_uid_is_known_wherever_this_process_can_read_about_itself() {
    assert_eq!(host_uid().is_some(), Path::new(OWN_PROCESS_DIR).exists());
}

#[test]
fn the_host_runtime_directory_follows_the_environment_and_the_owner() {
    let declared = std::env::var(RUNTIME_DIR_VARIABLE).ok();
    assert_eq!(
        host_runtime_dir(),
        runtime_dir_of(declared.as_deref(), host_uid())
    );
}

#[cfg(target_os = "linux")]
#[test]
fn the_host_uid_owns_this_very_process() {
    let uid = host_uid().expect("a unix process always has an owner");
    let own = std::fs::metadata(OWN_PROCESS_DIR).expect("a unix process can read about itself");
    assert_eq!(uid, own.uid());
}

#[cfg(target_os = "linux")]
#[test]
fn the_host_runtime_directory_is_known_even_without_a_login_session() {
    let dir = host_runtime_dir().expect("a unix process always has an owner");
    assert!(
        !dir.is_empty(),
        "systemctl --user needs a runtime directory"
    );
}

#[tokio::test]
async fn preparing_the_layout_creates_the_log_directory() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = resolve_paths(&dir.path().join("home"));
    let cfg_dir = dir.path().join("service");
    ensure_layout(&paths, &cfg_dir)
        .await
        .expect("should prepare");
    assert!(paths.logs_dir.is_dir());
}

#[tokio::test]
async fn preparing_the_layout_creates_the_service_directory() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = resolve_paths(&dir.path().join("home"));
    let cfg_dir = dir.path().join("config/pm3");
    ensure_layout(&paths, &cfg_dir)
        .await
        .expect("should prepare");
    assert!(cfg_dir.is_dir());
}

#[tokio::test]
async fn preparing_the_layout_keeps_the_service_directory_to_its_owner() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = resolve_paths(&dir.path().join("home"));
    let cfg_dir = dir.path().join("config/pm3");
    ensure_layout(&paths, &cfg_dir)
        .await
        .expect("should prepare");
    let mode = std::fs::metadata(&cfg_dir)
        .expect("read the metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o700, "the service directory holds the env files");
}

#[tokio::test]
async fn preparing_the_layout_reports_a_blocked_service_directory() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = resolve_paths(&dir.path().join("home"));
    let cfg_dir = dir.path().join("blocked");
    std::fs::write(&cfg_dir, "blocked").expect("occupy the service directory");
    let err = ensure_layout(&paths, &cfg_dir)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot prepare the pm3 home"), "got: {err}");
}

#[tokio::test]
async fn preparing_the_layout_reports_a_blocked_root() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path().join("home");
    std::fs::write(&root, "blocked").expect("occupy the root");
    let err = ensure_layout(&resolve_paths(&root), &root.join("service"))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot prepare the pm3 home"), "got: {err}");
}

#[tokio::test]
async fn preparing_the_layout_reports_a_blocked_log_directory() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = resolve_paths(&dir.path().join("home"));
    std::fs::create_dir_all(&paths.root).expect("create the root");
    std::fs::write(&paths.logs_dir, "blocked").expect("occupy the log directory");
    let err = ensure_layout(&paths, &dir.path().join("service"))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot prepare the pm3 home"), "got: {err}");
}

#[tokio::test]
async fn preparing_the_layout_restricts_the_home_to_its_owner() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = resolve_paths(&dir.path().join("home"));
    ensure_layout(&paths, &dir.path().join("service"))
        .await
        .expect("should prepare");
    let mode = std::fs::metadata(&paths.root)
        .expect("stat the home")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o700, "got: {mode:o}");
}

#[tokio::test]
async fn a_directory_that_cannot_be_restricted_only_warns() {
    let dir = tempfile::tempdir().expect("temp dir");
    let absent = dir.path().join("absent");
    restrict_to_owner(&absent).await;
    assert!(
        !absent.exists(),
        "a directory pm3 does not own must not stop it from running"
    );
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
async fn the_recorded_pid_can_be_read_back() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = resolve_paths(dir.path());
    write_pid_file(&paths).await.expect("should write");
    assert_eq!(read_pid_file(&paths).await, Some(std::process::id()));
}

#[tokio::test]
async fn a_missing_pid_file_reads_as_nothing() {
    let dir = tempfile::tempdir().expect("temp dir");
    assert_eq!(read_pid_file(&resolve_paths(dir.path())).await, None);
}

#[tokio::test]
async fn a_garbled_pid_file_reads_as_nothing() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = resolve_paths(dir.path());
    std::fs::write(&paths.pid_file, "not a pid").expect("seed a garbled pid file");
    assert_eq!(read_pid_file(&paths).await, None);
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

#[test]
fn the_service_directory_comes_from_the_config() {
    let mut config = pm3_config_with_home("/srv/pm3");
    config.cfg_dir = "~/.config/pm3".to_string();
    let resolved = resolve_cfg_dir(&config, Some("/home/dev")).expect("tilde expands");
    assert_eq!(resolved, std::path::Path::new("/home/dev/.config/pm3"));
}

#[test]
fn a_relative_service_directory_is_rejected() {
    let mut config = pm3_config_with_home("/srv/pm3");
    config.cfg_dir = "relative/service".to_string();
    let err = resolve_cfg_dir(&config, Some("/home/dev"))
        .unwrap_err()
        .to_string();
    assert!(err.contains("must be absolute"), "got: {err}");
}
