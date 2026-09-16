use super::*;
use crate::test_support::pm3_config_with_home;

#[test]
fn an_absolute_home_becomes_the_layout_root() {
    let paths = resolve_places(&pm3_config_with_home("/srv/pm3"), None)
        .expect("should resolve")
        .paths;
    assert_eq!(paths.socket, Path::new("/srv/pm3/pm3.sock"));
}

#[test]
fn a_tilde_home_is_expanded_against_the_environment() {
    let paths = resolve_places(&pm3_config_with_home("~/.pm3"), Some("/home/dev"))
        .expect("should resolve")
        .paths;
    assert_eq!(paths.dump_file, Path::new("/home/dev/.pm3/dump.yaml"));
}

#[test]
fn a_tilde_home_without_an_environment_is_rejected() {
    let err = resolve_places(&pm3_config_with_home("~/.pm3"), None)
        .unwrap_err()
        .to_string();
    assert!(err.contains("no HOME in the environment"), "got: {err}");
}

#[test]
fn the_host_home_comes_from_the_environment() {
    assert_eq!(host_home(), std::env::var("HOME").ok());
}

#[cfg(unix)]
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

#[cfg(unix)]
#[test]
fn the_host_uid_is_unknown_without_a_home_or_a_process_directory() {
    assert_eq!(
        host_uid(Some("/nonexistent/pm3-home")).is_some(),
        Path::new(OWN_PROCESS_DIR).exists()
    );
}

#[cfg(unix)]
#[test]
fn the_host_uid_falls_back_to_the_owner_of_the_home_directory() {
    use std::os::unix::fs::MetadataExt as _;

    let home = tempfile::tempdir().expect("temp dir");
    let uid = host_uid(home.path().to_str()).expect("this platform always reports an owner");
    let own = std::fs::metadata(home.path()).expect("a directory pm3 just made is readable");
    assert_eq!(uid, own.uid());
}

#[test]
fn the_host_runtime_directory_follows_the_environment_and_the_owner() {
    let declared = std::env::var(RUNTIME_DIR_VARIABLE).ok();
    assert_eq!(
        host_runtime_dir(None),
        runtime_dir_of(declared.as_deref(), host_uid(None))
    );
}

#[cfg(target_os = "linux")]
#[test]
fn the_host_uid_owns_this_very_process() {
    let uid = host_uid(None).expect("a unix process always has an owner");
    let own = std::fs::metadata(OWN_PROCESS_DIR).expect("a unix process can read about itself");
    assert_eq!(uid, own.uid());
}

#[cfg(target_os = "linux")]
#[test]
fn the_host_runtime_directory_is_known_even_without_a_login_session() {
    let dir = host_runtime_dir(None).expect("a unix process always has an owner");
    assert!(
        !dir.is_empty(),
        "systemctl --user needs a runtime directory"
    );
}

#[tokio::test]
async fn preparing_the_layout_creates_the_log_directory() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = resolve_paths(Pm3Roots::single(&dir.path().join("home")));
    let cfg_dir = dir.path().join("service");
    ensure_layout(&paths, &cfg_dir)
        .await
        .expect("should prepare");
    assert!(paths.logs_dir.is_dir());
}

#[tokio::test]
async fn preparing_the_layout_creates_the_service_directory() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = resolve_paths(Pm3Roots::single(&dir.path().join("home")));
    let cfg_dir = dir.path().join("config/pm3");
    ensure_layout(&paths, &cfg_dir)
        .await
        .expect("should prepare");
    assert!(cfg_dir.is_dir());
}

#[cfg(unix)]
#[tokio::test]
async fn preparing_the_layout_keeps_the_service_directory_to_its_owner() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = resolve_paths(Pm3Roots::single(&dir.path().join("home")));
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
    let paths = resolve_paths(Pm3Roots::single(&dir.path().join("home")));
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
    let err = ensure_layout(
        &resolve_paths(Pm3Roots::single(&root)),
        &root.join("service"),
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(err.contains("cannot prepare the pm3 home"), "got: {err}");
}

#[tokio::test]
async fn preparing_the_layout_reports_a_blocked_log_directory() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = resolve_paths(Pm3Roots::single(&dir.path().join("home")));
    std::fs::create_dir_all(&paths.root).expect("create the root");
    std::fs::write(&paths.logs_dir, "blocked").expect("occupy the log directory");
    let err = ensure_layout(&paths, &dir.path().join("service"))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot prepare the pm3 home"), "got: {err}");
}

#[cfg(unix)]
#[tokio::test]
async fn preparing_the_layout_restricts_the_home_to_its_owner() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = resolve_paths(Pm3Roots::single(&dir.path().join("home")));
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
    let paths = resolve_paths(Pm3Roots::single(dir.path()));
    write_pid_file(&paths).await.expect("should write");
    let recorded = std::fs::read_to_string(&paths.pid_file).expect("read");
    assert_eq!(recorded, std::process::id().to_string());
}

#[tokio::test]
async fn a_blocked_pid_path_is_reported() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = resolve_paths(Pm3Roots::single(dir.path()));
    std::fs::create_dir(&paths.pid_file).expect("occupy the pid path");
    let err = write_pid_file(&paths).await.unwrap_err().to_string();
    assert!(err.contains("cannot prepare the pm3 home"), "got: {err}");
}

#[tokio::test]
async fn the_recorded_pid_can_be_read_back() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = resolve_paths(Pm3Roots::single(dir.path()));
    write_pid_file(&paths).await.expect("should write");
    assert_eq!(read_pid_file(&paths).await, Some(std::process::id()));
}

#[tokio::test]
async fn a_missing_pid_file_reads_as_nothing() {
    let dir = tempfile::tempdir().expect("temp dir");
    assert_eq!(
        read_pid_file(&resolve_paths(Pm3Roots::single(dir.path()))).await,
        None
    );
}

#[tokio::test]
async fn a_garbled_pid_file_reads_as_nothing() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = resolve_paths(Pm3Roots::single(dir.path()));
    std::fs::write(&paths.pid_file, "not a pid").expect("seed a garbled pid file");
    assert_eq!(read_pid_file(&paths).await, None);
}

#[tokio::test]
async fn clearing_runtime_files_removes_the_socket_and_the_pid_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = resolve_paths(Pm3Roots::single(dir.path()));
    std::fs::write(&paths.socket, "socket").expect("seed socket");
    write_pid_file(&paths).await.expect("should write");
    clear_runtime_files(&paths).await;
    assert!(!paths.socket.exists() && !paths.pid_file.exists());
}

#[tokio::test]
async fn clearing_runtime_files_tolerates_missing_files() {
    let dir = tempfile::tempdir().expect("temp dir");
    clear_runtime_files(&resolve_paths(Pm3Roots::single(dir.path()))).await;
    assert!(!dir.path().join("pm3.sock").exists());
}

#[tokio::test]
async fn a_runtime_file_that_cannot_be_removed_only_warns() {
    let dir = tempfile::tempdir().expect("temp dir");
    let paths = resolve_paths(Pm3Roots::single(dir.path()));
    std::fs::create_dir(&paths.pid_file).expect("occupy the pid path");
    clear_runtime_files(&paths).await;
    assert!(
        paths.pid_file.exists(),
        "a removal failure must not stop the daemon from finishing its shutdown"
    );
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

#[cfg(windows)]
#[test]
fn a_pipe_name_changes_with_the_secret() {
    let socket = Path::new(r"C:\pm3\pm3.sock");
    let first = pipe_name_of(socket, "0123456789abcdef0123456789abcdef");
    let second = pipe_name_of(socket, "fedcba9876543210fedcba9876543210");
    assert_ne!(first, second);
    assert!(first.starts_with(r"\\.\pipe\pm3-"), "got: {first}");
}

#[cfg(windows)]
#[test]
fn a_generated_secret_has_the_shape_of_a_pipe_secret() {
    let secret = generate_pipe_secret();
    assert!(is_pipe_secret(&secret), "got: {secret}");
}

#[cfg(windows)]
#[test]
fn a_secret_with_a_bad_shape_is_rejected() {
    assert!(!is_pipe_secret("too-short"));
    assert!(!is_pipe_secret("0123456789abcdef0123456789abcdeg"));
    assert!(!is_pipe_secret("0123456789ABCDEF0123456789ABCDEF0"));
}

#[cfg(windows)]
#[tokio::test]
async fn a_missing_pipe_secret_is_created_once_and_reused() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("pm3.sock");
    let first = pipe_secret(&socket).await.expect("create the secret");
    let second = pipe_secret(&socket).await.expect("read the secret back");
    assert_eq!(first, second);
    assert!(is_pipe_secret(&first), "got: {first}");
}

#[cfg(windows)]
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

#[cfg(windows)]
#[tokio::test]
async fn a_pipe_secret_in_a_missing_home_is_reported() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("absent").join("pm3.sock");
    let err = pipe_secret(&socket).await.unwrap_err().to_string();
    assert!(err.contains("absent"), "got: {err}");
}

#[cfg(windows)]
#[tokio::test]
async fn a_pipe_secret_blocked_by_a_directory_is_reported() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("pm3.sock");
    std::fs::create_dir(socket.with_file_name(PIPE_SECRET_FILE)).expect("block the secret path");
    let err = pipe_secret(&socket).await.unwrap_err().to_string();
    assert!(err.contains(PIPE_SECRET_FILE), "got: {err}");
}

#[cfg(windows)]
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

#[test]
fn a_declared_cfg_dir_survives_the_config_root_variable() {
    let mut config = pm3_config_with_home("/srv/pm3");
    config.cfg_dir = "/srv/pm3/service".to_string();
    let cfg_dir = resolve_cfg_dir(&config, None).expect("the cfg dir should resolve");
    assert_eq!(
        cfg_dir,
        Path::new("/srv/pm3/service"),
        "the config root and the service directory are different settings"
    );
}

fn split_config() -> adapters::Pm3Config {
    let mut config = pm3_config_with_home("/srv/pm3");
    config.home = String::new();
    config.cfg_dir = String::new();
    config.state_dir = "/s/pm3".to_string();
    config.runtime_dir = "/r/pm3".to_string();
    config.data_dir = "/d/pm3".to_string();
    config
}

#[test]
fn an_empty_home_derives_the_split_layout_from_the_configured_roots() {
    let paths = resolve_places(&split_config(), Some("/home/dev"))
        .expect("should resolve")
        .paths;
    assert_eq!(paths.dump_file, Path::new("/s/pm3/dump.yaml"));
    assert_eq!(paths.socket, Path::new("/r/pm3/pm3.sock"));
    assert_eq!(paths.backups_dir, Path::new("/d/pm3/install-backups"));
    assert_eq!(paths.apps_dir, Path::new("/s/pm3/apps"));
}

#[test]
fn an_empty_cfg_dir_follows_the_config_root() {
    let cfg_dir =
        resolve_cfg_dir(&split_config(), Some("/home/dev")).expect("the cfg dir should resolve");
    assert!(
        cfg_dir.ends_with("pm3"),
        "the service directory follows the config root, got {}",
        cfg_dir.to_string_lossy()
    );
}

#[test]
fn a_relative_state_root_is_refused() {
    let mut config = split_config();
    config.state_dir = "pm3-state".to_string();
    let err = resolve_places(&config, Some("/home/dev")).unwrap_err();
    assert!(err.to_string().contains("must be absolute"), "got: {err}");
}

#[test]
fn a_relative_home_is_refused() {
    let mut config = split_config();
    config.home = "pm3-data".to_string();
    let err = resolve_places(&config, Some("/home/dev")).unwrap_err();
    assert!(err.to_string().contains("must be absolute"), "got: {err}");
}

#[test]
fn a_socket_path_past_the_unix_limit_is_refused() {
    let mut config = split_config();
    config.runtime_dir = format!("/{}", "d".repeat(120));
    let err = resolve_places(&config, Some("/home/dev")).unwrap_err();
    assert!(err.to_string().contains("exceeds the 104"), "got: {err}");
}

#[test]
fn a_relative_cfg_dir_is_refused() {
    let mut config = split_config();
    config.cfg_dir = "service".to_string();
    let err = resolve_cfg_dir(&config, Some("/home/dev")).unwrap_err();
    assert!(err.to_string().contains("must be absolute"), "got: {err}");
}

#[test]
fn a_directory_that_exists_is_reported_as_one() {
    let dir = tempfile::tempdir().expect("temp dir");
    assert!(directory_exists(dir.path()));
    assert!(!directory_exists(&dir.path().join("absent")));
}

#[test]
fn an_empty_runtime_root_falls_back_to_the_state_root() {
    let mut config = split_config();
    config.runtime_dir = String::new();
    let paths = resolve_places(&config, Some("/home/dev"))
        .expect("should resolve")
        .paths;
    assert!(
        paths.socket.starts_with("/run/user") || paths.socket.starts_with("/s/pm3/run"),
        "the runtime root follows the xdg rules, got {}",
        paths.socket.to_string_lossy()
    );
}

fn bare_sources(home: Option<&str>) -> RootSources<'_> {
    RootSources {
        pm3_home: None,
        config: None,
        state: None,
        runtime: None,
        data: None,
        xdg_config: None,
        xdg_state: None,
        xdg_runtime: None,
        xdg_data: None,
        home,
        uid: None,
        exists: directory_exists,
    }
}

fn derived_config() -> adapters::Pm3Config {
    let mut config = pm3_config_with_home("/srv/pm3");
    config.home = String::new();
    config.cfg_dir = String::new();
    config
}

#[test]
fn deriving_the_roots_without_a_home_is_refused() {
    let err = roots_from(&derived_config(), &bare_sources(None)).unwrap_err();
    assert!(
        err.to_string().contains("no HOME in the environment"),
        "got: {err}"
    );
}

#[test]
fn deriving_the_roots_falls_back_to_the_xdg_defaults() {
    let roots = roots_from(&derived_config(), &bare_sources(Some("/home/dev")))
        .expect("the fallbacks resolve");
    assert_eq!(roots.config, Path::new("/home/dev/.config/pm3"));
    assert_eq!(roots.state, Path::new("/home/dev/.local/state/pm3"));
    assert_eq!(roots.data, Path::new("/home/dev/.local/share/pm3"));
    assert_eq!(
        roots.runtime,
        Path::new("/home/dev/.local/state/pm3/run"),
        "macos has no per-user runtime directory"
    );
}

#[test]
fn a_relative_data_root_is_refused_while_deriving() {
    let mut config = derived_config();
    config.data_dir = "share".to_string();
    let err = roots_from(&config, &bare_sources(Some("/home/dev"))).unwrap_err();
    assert!(err.to_string().contains("must be absolute"), "got: {err}");
}

#[test]
fn a_relative_config_root_variable_is_refused_while_deriving() {
    let mut sources = bare_sources(Some("/home/dev"));
    sources.config = Some("cfg");
    let err = roots_from(&derived_config(), &sources).unwrap_err();
    assert!(err.to_string().contains("must be absolute"), "got: {err}");
}

#[test]
fn a_pm3_home_variable_wins_over_every_derived_root() {
    let mut sources = bare_sources(Some("/home/dev"));
    sources.pm3_home = Some("/srv/pm3");
    let roots = roots_from(&derived_config(), &sources).expect("the single root resolves");
    assert_eq!(roots.state, Path::new("/srv/pm3"));
    assert_eq!(roots.runtime, Path::new("/srv/pm3"));
}
