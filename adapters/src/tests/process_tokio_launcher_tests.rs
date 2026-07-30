use tempfile::TempDir;

use super::{test_helpers::*, *};

fn temp_dir() -> TempDir {
    tempfile::tempdir().expect("create temp dir")
}

#[tokio::test]
async fn spawn_reports_the_child_pid() {
    let dir = temp_dir();
    let launcher = TokioProcessLauncher::default();
    let spec = spec_in(dir.path(), ECHO_PROGRAM, &["hello"]);
    let process = launcher.spawn(&spec).await.expect("should spawn");
    assert!(process.pid > 0, "got: {}", process.pid);
}

#[tokio::test]
async fn spawn_redirects_stdout_to_the_log_file() {
    let dir = temp_dir();
    let spec = spec_in(dir.path(), ECHO_PROGRAM, &["hello"]);
    run_to_completion(&spec).await.expect("should reap");
    assert_eq!(read_log(dir.path(), OUT_LOG).await, "hello\n");
}

#[tokio::test]
async fn spawn_redirects_stderr_to_the_log_file() {
    let dir = temp_dir();
    let spec = spec_in(dir.path(), SHELL_PROGRAM, &["-c", "echo oops >&2"]);
    run_to_completion(&spec).await.expect("should reap");
    assert_eq!(read_log(dir.path(), ERR_LOG).await, "oops\n");
}

#[tokio::test]
async fn spawn_appends_to_an_existing_log_file() {
    let dir = temp_dir();
    tokio::fs::write(dir.path().join(OUT_LOG), "earlier\n")
        .await
        .expect("seed the log");
    let spec = spec_in(dir.path(), ECHO_PROGRAM, &["later"]);
    run_to_completion(&spec).await.expect("should reap");
    assert_eq!(read_log(dir.path(), OUT_LOG).await, "earlier\nlater\n");
}

#[tokio::test]
async fn spawn_passes_the_declared_environment() {
    let dir = temp_dir();
    let mut spec = spec_in(
        dir.path(),
        SHELL_PROGRAM,
        &["-c", "echo $PM3_LAUNCHER_PROBE"],
    );
    spec.env = vec![("PM3_LAUNCHER_PROBE".to_string(), "visible".to_string())];
    run_to_completion(&spec).await.expect("should reap");
    assert_eq!(read_log(dir.path(), OUT_LOG).await, "visible\n");
}

#[tokio::test]
async fn spawn_runs_in_the_requested_directory() {
    let dir = temp_dir();
    let expected = dir.path().canonicalize().expect("canonical temp dir");
    let spec = spec_in(dir.path(), PWD_PROGRAM, &[]);
    run_to_completion(&spec).await.expect("should reap");
    let printed = read_log(dir.path(), OUT_LOG).await;
    assert_eq!(printed.trim_end(), text(&expected));
}

#[tokio::test]
async fn spawn_reports_a_missing_program() {
    let dir = temp_dir();
    let launcher = TokioProcessLauncher::default();
    let spec = spec_in(dir.path(), "/nonexistent/pm3-probe", &[]);
    let err = launcher.spawn(&spec).await.unwrap_err().to_string();
    assert!(err.contains("cannot spawn app 'web'"), "got: {err}");
}

#[tokio::test]
async fn spawn_reports_an_unopenable_stdout_log() {
    let dir = temp_dir();
    let launcher = TokioProcessLauncher::default();
    let mut spec = spec_in(dir.path(), ECHO_PROGRAM, &["hello"]);
    spec.stdout_path = text(&dir.path().join("absent").join(OUT_LOG));
    let err = launcher.spawn(&spec).await.unwrap_err().to_string();
    assert!(err.contains("cannot open log file"), "got: {err}");
}

#[tokio::test]
async fn spawn_reports_an_unopenable_stderr_log() {
    let dir = temp_dir();
    let launcher = TokioProcessLauncher::default();
    let mut spec = spec_in(dir.path(), ECHO_PROGRAM, &["hello"]);
    spec.stderr_path = text(&dir.path().join("absent").join(ERR_LOG));
    let err = launcher.spawn(&spec).await.unwrap_err().to_string();
    assert!(err.contains(ERR_LOG), "got: {err}");
}

#[tokio::test]
async fn wait_reports_a_clean_exit() {
    let dir = temp_dir();
    let spec = spec_in(dir.path(), ECHO_PROGRAM, &["hello"]);
    let outcome = run_to_completion(&spec).await.expect("should reap");
    assert!(outcome.clean(), "got: {outcome:?}");
}

#[tokio::test]
async fn wait_reports_a_failing_exit_code() {
    let dir = temp_dir();
    let spec = spec_in(dir.path(), SHELL_PROGRAM, &["-c", "exit 3"]);
    let outcome = run_to_completion(&spec).await.expect("should reap");
    assert_eq!(outcome.exit_code, Some(3));
}

#[tokio::test]
async fn wait_reports_no_code_for_a_signalled_child() {
    let dir = temp_dir();
    let launcher = TokioProcessLauncher::default();
    let spec = spec_in(dir.path(), SHELL_PROGRAM, &["-c", "sleep 30"]);
    let process = launcher.spawn(&spec).await.expect("should spawn");
    let killed = tokio::process::Command::new("/bin/kill")
        .args(["-KILL", &process.pid.to_string()])
        .status()
        .await
        .expect("should signal");
    assert!(killed.success(), "kill should succeed");
    let outcome = launcher.wait(process.pid).await.expect("should reap");
    assert_eq!(outcome.exit_code, None);
}

#[tokio::test]
async fn a_launcher_tracks_nothing_before_it_spawns() {
    let launcher = TokioProcessLauncher::default();
    assert!(launcher.tracked_pids().await.is_empty());
}

#[tokio::test]
async fn a_spawned_child_is_tracked_until_it_is_reaped() {
    let dir = temp_dir();
    let launcher = TokioProcessLauncher::default();
    let spec = spec_in(dir.path(), ECHO_PROGRAM, &["hello"]);
    let process = launcher.spawn(&spec).await.expect("should spawn");
    assert_eq!(launcher.tracked_pids().await, vec![process.pid]);
    launcher.wait(process.pid).await.expect("reap");
    assert!(launcher.tracked_pids().await.is_empty());
}

#[tokio::test]
async fn wait_reports_nothing_for_an_untracked_pid() {
    let launcher = TokioProcessLauncher::default();
    assert!(launcher.wait(1).await.is_none());
}

#[tokio::test]
async fn wait_forgets_a_child_once_reaped() {
    let dir = temp_dir();
    let launcher = TokioProcessLauncher::default();
    let spec = spec_in(dir.path(), ECHO_PROGRAM, &["hello"]);
    let process = launcher.spawn(&spec).await.expect("should spawn");
    launcher.wait(process.pid).await.expect("first reap");
    assert!(launcher.wait(process.pid).await.is_none());
}
