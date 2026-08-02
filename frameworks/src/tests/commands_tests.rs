use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::UnixListener,
    task::JoinHandle,
};

use super::*;
use crate::daemon_fixture::{
    Collected, Fixture, running_daemon, seed_log, sleeper_apps_file, stop_daemon,
};

#[test]
fn a_missing_apps_file_cannot_be_resolved() {
    let err = canonical_apps_file("/nonexistent/apps.yaml")
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot resolve the apps file"), "got: {err}");
}

#[test]
fn an_apps_file_is_resolved_to_an_absolute_path() {
    let dir = tempfile::tempdir().expect("temp dir");
    let file = crate::test_support::write_apps_file(dir.path(), "apps: []\n");
    let resolved = canonical_apps_file(file.to_str().expect("path")).expect("should resolve");
    assert!(resolved.starts_with('/'), "got: {resolved}");
}

#[test]
fn a_session_cannot_open_without_a_config() {
    assert!(open_session("/nonexistent/pm3.yaml").is_err());
}

#[test]
fn checking_a_missing_config_fails() {
    assert!(check_config("/nonexistent/pm3.yaml").is_err());
}

#[test]
fn showing_a_missing_config_fails() {
    assert!(show_config("/nonexistent/pm3.yaml").is_err());
}

#[tokio::test]
async fn sleeping_returns_after_the_requested_delay() {
    sleep_for(1).await;
}

#[tokio::test]
async fn listing_an_empty_daemon_reports_that_nothing_runs() {
    let fixture = running_daemon().await;
    let listed = list_apps(&fixture.config_path).await.expect("should list");
    assert!(listed.contains("no apps"), "got: {listed}");
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn starting_an_apps_file_reports_the_started_app() {
    let fixture = running_daemon().await;
    let apps_file = sleeper_apps_file(&fixture);
    let started = start_apps(&fixture.config_path, &apps_file, false)
        .await
        .expect("should start");
    assert!(started.response.contains("started web"), "got: {started:?}");
    assert_eq!(started.changed, vec!["web".to_string()]);
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn restarting_an_unchanged_apps_file_reports_no_config_change() {
    let fixture = running_daemon().await;
    let apps_file = sleeper_apps_file(&fixture);
    start_apps(&fixture.config_path, &apps_file, false)
        .await
        .expect("should start");
    let again = start_apps(&fixture.config_path, &apps_file, false)
        .await
        .expect("should start");
    assert!(again.changed.is_empty(), "got: {:?}", again.changed);
    assert!(
        again.response.contains("already running"),
        "got: {}",
        again.response
    );
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn restarting_a_changed_apps_file_reports_the_changed_app() {
    let fixture = running_daemon().await;
    let apps_file = sleeper_apps_file(&fixture);
    start_apps(&fixture.config_path, &apps_file, false)
        .await
        .expect("should start");
    let cwd = fixture.paths.root.to_string_lossy();
    std::fs::write(
        &apps_file,
        format!(
            "apps:\n  - name: web\n    script: /bin/sh\n    cwd: \"{cwd}\"\n    args:\n      - \"-c\"\n      - \"sleep 60\"\n"
        ),
    )
    .expect("edit the apps file");
    let again = start_apps(&fixture.config_path, &apps_file, true)
        .await
        .expect("should start");
    assert_eq!(again.changed, vec!["web".to_string()]);
    assert!(
        again.response.contains("already running"),
        "got: {}",
        again.response
    );
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn describing_a_started_app_reports_its_script() {
    let fixture = running_daemon().await;
    let apps_file = sleeper_apps_file(&fixture);
    start_apps(&fixture.config_path, &apps_file, false)
        .await
        .expect("should start");
    let described = describe_app(&fixture.config_path, "web")
        .await
        .expect("should describe");
    assert!(described.contains("/bin/sh"), "got: {described}");
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn stopping_a_started_app_confirms_it() {
    let fixture = running_daemon().await;
    let apps_file = sleeper_apps_file(&fixture);
    start_apps(&fixture.config_path, &apps_file, false)
        .await
        .expect("should start");
    let stopped = act_on_app(&fixture.config_path, "web", STOP_ACTION)
        .await
        .expect("should stop");
    assert_eq!(stopped, "stopped web");
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn a_selector_that_would_break_the_request_line_is_refused_before_dialling() {
    let error = act_on_app("/nonexistent/config.yaml", "my app", STOP_ACTION)
        .await
        .unwrap_err();
    assert!(matches!(error, Error::Spec(_)), "got: {error}");
}

#[tokio::test]
async fn a_selector_that_would_escape_the_apps_path_is_refused_before_dialling() {
    let error = describe_app("/nonexistent/config.yaml", "../health")
        .await
        .unwrap_err();
    assert!(matches!(error, Error::Spec(_)), "got: {error}");
}

#[tokio::test]
async fn deleting_with_a_selector_that_would_escape_the_apps_path_is_refused() {
    let error = delete_app("/nonexistent/config.yaml", "my app")
        .await
        .unwrap_err();
    assert!(matches!(error, Error::Spec(_)), "got: {error}");
}

#[tokio::test]
async fn describing_without_a_config_is_reported() {
    let error = describe_app("/nonexistent/config.yaml", "web")
        .await
        .unwrap_err();
    assert!(!matches!(error, Error::Spec(_)), "got: {error}");
}

#[tokio::test]
async fn deleting_an_unknown_app_carries_the_daemon_reason() {
    let fixture = running_daemon().await;
    let err = delete_app(&fixture.config_path, "ghost")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("status 404"), "got: {err}");
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn killing_with_services_reports_a_dump_it_cannot_write() {
    let fixture = running_daemon().await;
    let apps_file = sleeper_apps_file(&fixture);
    start_apps(&fixture.config_path, &apps_file, false)
        .await
        .expect("should start");
    std::fs::remove_file(&fixture.paths.dump_file).expect("drop the dump file");
    std::fs::create_dir_all(&fixture.paths.dump_file).expect("block the dump path");
    std::fs::write(fixture.paths.dump_file.join("occupied"), "state")
        .expect("fill the blocked dump path");

    let err = kill_daemon(&fixture.config_path, true)
        .await
        .unwrap_err()
        .to_string();

    assert!(err.contains("status 500"), "got: {err}");
    std::fs::remove_dir_all(&fixture.paths.dump_file).expect("unblock the dump path");
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn a_numeric_selector_still_reaches_the_daemon() {
    let error = delete_app("/nonexistent/config.yaml", "3")
        .await
        .unwrap_err();
    assert!(!matches!(error, Error::Spec(_)), "got: {error}");
}

#[tokio::test]
async fn restarting_a_started_app_confirms_it() {
    let fixture = running_daemon().await;
    let apps_file = sleeper_apps_file(&fixture);
    start_apps(&fixture.config_path, &apps_file, false)
        .await
        .expect("should start");
    let restarted = act_on_app(&fixture.config_path, "web", RESTART_ACTION)
        .await
        .expect("should restart");
    assert_eq!(restarted, "restarted web");
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn deleting_a_started_app_confirms_it() {
    let fixture = running_daemon().await;
    let apps_file = sleeper_apps_file(&fixture);
    start_apps(&fixture.config_path, &apps_file, false)
        .await
        .expect("should start");
    let deleted = delete_app(&fixture.config_path, "web")
        .await
        .expect("should delete");
    assert_eq!(deleted, "deleted web");
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn a_refused_request_carries_the_daemon_reason() {
    let fixture = running_daemon().await;
    let err = describe_app(&fixture.config_path, "ghost")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("status 404"), "got: {err}");
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn listing_without_a_config_fails() {
    assert!(list_apps("/nonexistent/pm3.yaml").await.is_err());
}

#[tokio::test]
async fn starting_a_missing_apps_file_fails_before_any_daemon_call() {
    let outcome = start_apps("/nonexistent/pm3.yaml", "/nonexistent/apps.yaml", false).await;
    assert!(outcome.is_err(), "got: {outcome:?}");
}

#[test]
fn a_relative_home_cannot_be_resolved() {
    let dir = tempfile::tempdir().expect("temp dir");
    let config = crate::test_support::write_config(dir.path(), "relative/home");
    let err = open_session(config.to_str().expect("path"))
        .unwrap_err()
        .to_string();
    assert!(err.contains("must be absolute"), "got: {err}");
}

#[tokio::test]
async fn a_blocked_home_stops_a_command() {
    let dir = tempfile::tempdir().expect("temp dir");
    let blocked = dir.path().join("blocked");
    std::fs::write(&blocked, "occupied").expect("occupy the parent");
    let home = blocked.join("home");
    let config = crate::test_support::write_config(dir.path(), &home.to_string_lossy());
    let err = list_apps(config.to_str().expect("path"))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot prepare the pm3 home"), "got: {err}");
}

#[tokio::test]
async fn a_daemon_that_never_comes_up_stops_a_command() {
    let dir = tempfile::tempdir().expect("temp dir");
    let home = dir.path().join("home");
    let config = crate::test_support::write_impatient_config(dir.path(), &home.to_string_lossy());
    let err = list_apps(config.to_str().expect("path"))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot reach the pm3 daemon"), "got: {err}");
}

#[tokio::test]
async fn a_daemon_that_disappears_after_the_probe_stops_a_command() {
    let dir = tempfile::tempdir().expect("temp dir");
    let home = dir.path().join("home");
    std::fs::create_dir_all(home.join("logs")).expect("prepare the home");
    let config = crate::test_support::write_config(dir.path(), &home.to_string_lossy());
    let answering = crate::daemon_fixture::answer_only_the_health_probe(home.join("pm3.sock"));
    let err = list_apps(config.to_str().expect("path"))
        .await
        .unwrap_err()
        .to_string();
    answering.await.expect("join the probe answerer");
    assert!(
        err.contains("cannot connect to the pm3 daemon"),
        "got: {err}"
    );
}

fn blocked_home_config(dir: &std::path::Path) -> String {
    let home = dir.join("home");
    std::fs::write(&home, "blocked").expect("occupy the pm3 home");
    crate::test_support::write_config(dir, &home.to_string_lossy())
        .to_string_lossy()
        .into_owned()
}

fn usable_config(dir: &std::path::Path) -> String {
    let home = dir.join("home");
    crate::test_support::write_config(dir, &home.to_string_lossy())
        .to_string_lossy()
        .into_owned()
}

fn inline_request<'s>(program: &'s str, args: &'s [String]) -> InlineStart<'s> {
    InlineStart {
        name: "probe",
        program,
        args,
        cwd: None,
        env: &[],
        cron: None,
        autorestart: None,
        network: false,
        writable_dirs: &[],
        force: false,
    }
}

#[tokio::test]
async fn starting_apps_reports_a_blocked_home() {
    let dir = tempfile::tempdir().expect("temp dir");
    let config = blocked_home_config(dir.path());
    let err = start_apps(&config, "/nonexistent/apps.yaml", false)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot prepare the pm3 home"), "got: {err}");
}

#[tokio::test]
async fn starting_apps_reports_an_unresolvable_apps_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let config = usable_config(dir.path());
    let err = start_apps(&config, "/nonexistent/apps.yaml", false)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot resolve the apps file"), "got: {err}");
}

#[tokio::test]
async fn starting_apps_reports_an_unreadable_apps_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let config = usable_config(dir.path());
    let blocked = dir.path().join("apps-as-a-directory");
    std::fs::create_dir(&blocked).expect("occupy the apps file path");
    let err = start_apps(&config, &blocked.to_string_lossy(), false)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot read apps file"), "got: {err}");
}

#[tokio::test]
async fn starting_inline_without_a_config_fails() {
    let outcome = start_inline("/nonexistent/pm3.yaml", &inline_request("/bin/sh", &[])).await;
    assert!(outcome.is_err(), "got: {outcome:?}");
}

#[tokio::test]
async fn starting_inline_with_a_program_off_the_search_path_fails() {
    let dir = tempfile::tempdir().expect("temp dir");
    let config = usable_config(dir.path());
    let err = start_inline(&config, &inline_request("pm3-not-a-real-program", &[]))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot find"), "got: {err}");
}

#[tokio::test]
async fn starting_inline_reports_a_blocked_home() {
    let dir = tempfile::tempdir().expect("temp dir");
    let config = blocked_home_config(dir.path());
    let err = start_inline(&config, &inline_request("/bin/sh", &[]))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot prepare the pm3 home"), "got: {err}");
}

#[tokio::test]
async fn deleting_without_a_config_fails() {
    assert!(delete_app("/nonexistent/pm3.yaml", "web").await.is_err());
}

#[tokio::test]
async fn deleting_an_unknown_app_fails() {
    let fixture = running_daemon().await;
    let outcome = delete_app(&fixture.config_path, "ghost").await;
    assert!(outcome.is_err(), "got: {outcome:?}");
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn starting_inline_reaches_the_daemon() {
    let fixture = running_daemon().await;
    let args = vec!["-c".to_string(), "sleep 5".to_string()];
    let started = start_inline(&fixture.config_path, &inline_request("/bin/sh", &args))
        .await
        .expect("the inline app should start");
    assert!(
        started.response.contains("started probe"),
        "got: {started:?}"
    );
    assert_eq!(started.changed, vec!["probe".to_string()]);
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn restarting_an_unchanged_inline_app_reports_no_config_change() {
    let fixture = running_daemon().await;
    let args = vec!["-c".to_string(), "sleep 5".to_string()];
    start_inline(&fixture.config_path, &inline_request("/bin/sh", &args))
        .await
        .expect("should start");
    let again = start_inline(&fixture.config_path, &inline_request("/bin/sh", &args))
        .await
        .expect("should start");
    assert!(again.changed.is_empty(), "got: {:?}", again.changed);
    assert!(
        again.response.contains("already running"),
        "got: {}",
        again.response
    );
    stop_daemon(fixture).await;
}

#[test]
fn a_relative_service_directory_cannot_open_a_session() {
    let dir = tempfile::tempdir().expect("temp dir");
    let config = crate::test_support::write_config_with_cfg_dir(
        dir.path(),
        "/tmp/pm3-service",
        "relative/service",
    );
    let err = open_session(config.to_str().expect("path"))
        .unwrap_err()
        .to_string();
    assert!(err.contains("must be absolute"), "got: {err}");
}

const HEALTH_REPLY: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok";
const STOP_ALL_REPLY: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 24\r\n\r\n{\"report\":\"stopped all\"}";
const SCRIPT_SINK: usize = 1024;

fn vanishing_daemon(socket: PathBuf, replies: &'static [&'static [u8]]) -> JoinHandle<()> {
    let listener = UnixListener::bind(&socket).expect("bind the scripted daemon");
    tokio::spawn(async move {
        for reply in replies {
            let Ok((mut stream, _addr)) = listener.accept().await else {
                break;
            };
            let mut sink = vec![0_u8; SCRIPT_SINK];
            let read = stream.read(&mut sink).await.unwrap_or_default();
            sink.truncate(read);
            stream.write_all(reply).await.ok();
            stream.shutdown().await.ok();
        }
        drop(listener);
        std::fs::remove_file(&socket).ok();
    })
}

#[tokio::test]
async fn killing_a_daemon_whose_pid_file_vanished_reports_the_loss() {
    let fixture = running_daemon().await;
    std::fs::remove_file(&fixture.paths.pid_file).expect("drop the pid file");
    let err = kill_daemon(&fixture.config_path, false)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot read the pm3 daemon pid"), "got: {err}");
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn killing_a_daemon_that_already_left_is_treated_as_stopped() {
    let dir = tempfile::tempdir().expect("temp dir");
    let home = dir.path().join("home");
    std::fs::create_dir_all(home.join("logs")).expect("prepare the home");
    let config = crate::test_support::write_config(dir.path(), &home.to_string_lossy());
    let answering = crate::daemon_fixture::answer_only_the_health_probe(home.join("pm3.sock"));
    let gone = kill_daemon(config.to_str().expect("path"), false)
        .await
        .expect("a daemon that already left counts as stopped");
    answering.await.expect("join the probe answerer");
    assert_eq!(gone, DAEMON_NOT_RUNNING);
}

#[tokio::test]
async fn killing_with_services_reports_them_even_when_the_daemon_left_mid_kill() {
    let dir = tempfile::tempdir().expect("temp dir");
    let home = dir.path().join("home");
    std::fs::create_dir_all(home.join("logs")).expect("prepare the home");
    let config = crate::test_support::write_config(dir.path(), &home.to_string_lossy());
    vanishing_daemon(
        home.join("pm3.sock"),
        &[HEALTH_REPLY, HEALTH_REPLY, STOP_ALL_REPLY],
    );
    let gone = kill_daemon(config.to_str().expect("path"), true)
        .await
        .expect("stop-all succeeded, so the vanished daemon counts as stopped");
    assert!(gone.contains("stopped all"), "got: {gone}");
    assert!(gone.contains("not running"), "got: {gone}");
}

#[tokio::test]
async fn a_command_that_cannot_locate_the_pm3_binary_is_reported() {
    let dir = tempfile::tempdir().expect("temp dir");
    let home = dir.path().join("home");
    let config = crate::test_support::write_config(dir.path(), &home.to_string_lossy());
    let session = open_session(config.to_str().expect("path")).expect("open a session");
    let broken: std::io::Result<PathBuf> = Err(std::io::Error::other("image gone"));
    let err = ask_with(&session, "GET", APPS_PATH, None, &broken)
        .await
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("cannot determine the pm3 binary path"),
        "got: {err}"
    );
}

#[path = "commands_safety_tests.rs"]
mod safety;

#[path = "commands_logs_tests.rs"]
mod logs;
