use super::*;
use crate::daemon_fixture::{Collected, running_daemon, seed_log, sleeper_apps_file, stop_daemon};

#[test]
fn a_start_body_carries_the_apps_file() {
    assert_eq!(
        start_body("/srv/apps.yaml"),
        "{\"apps_file\":\"/srv/apps.yaml\"}"
    );
}

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
    let started = start_apps(&fixture.config_path, &apps_file)
        .await
        .expect("should start");
    assert!(started.contains("started web"), "got: {started}");
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn describing_a_started_app_reports_its_script() {
    let fixture = running_daemon().await;
    let apps_file = sleeper_apps_file(&fixture);
    start_apps(&fixture.config_path, &apps_file)
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
    start_apps(&fixture.config_path, &apps_file)
        .await
        .expect("should start");
    let stopped = stop_app(&fixture.config_path, "web")
        .await
        .expect("should stop");
    assert_eq!(stopped, "stopped web");
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn restarting_a_started_app_confirms_it() {
    let fixture = running_daemon().await;
    let apps_file = sleeper_apps_file(&fixture);
    start_apps(&fixture.config_path, &apps_file)
        .await
        .expect("should start");
    let restarted = restart_app(&fixture.config_path, "web")
        .await
        .expect("should restart");
    assert_eq!(restarted, "restarted web");
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn deleting_a_started_app_confirms_it() {
    let fixture = running_daemon().await;
    let apps_file = sleeper_apps_file(&fixture);
    start_apps(&fixture.config_path, &apps_file)
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
async fn reading_a_log_tail_returns_the_last_lines() {
    let fixture = running_daemon().await;
    seed_log(&fixture, "web", "first\nsecond\nthird\n");
    let tail = read_log_tail(&fixture.config_path, "web", 2)
        .await
        .expect("should read");
    assert_eq!(tail, "second\nthird");
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn reading_a_missing_log_fails() {
    let fixture = running_daemon().await;
    let err = read_log_tail(&fixture.config_path, "ghost", 5)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot read log file"), "got: {err}");
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn following_a_log_emits_the_lines_appended_after_it_started() {
    let fixture = running_daemon().await;
    let path = seed_log(&fixture, "web", "old\n");
    let collected = Collected::default();
    let appended = path.clone();
    let writer = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(60)).await;
        std::fs::write(&appended, "old\nfresh\n").expect("append a line");
    });
    follow_log(&fixture.config_path, "web", 2, &|line| {
        collected.push(line);
    })
    .await
    .expect("should follow");
    writer.await.expect("join the writer");
    assert_eq!(collected.taken(), vec!["fresh"]);
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn following_a_missing_log_fails() {
    let fixture = running_daemon().await;
    let outcome = follow_log(&fixture.config_path, "ghost", 1, &|_line| {}).await;
    assert!(outcome.is_err(), "got: {outcome:?}");
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn following_a_log_that_turns_undecodable_fails() {
    let fixture = running_daemon().await;
    let path = seed_log(&fixture, "web", "old\n");
    let appended = path.clone();
    let writer = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(60)).await;
        std::fs::write(&appended, [b'o', b'l', b'd', b'\n', 0xff, 0xfe, b'\n'])
            .expect("append raw bytes");
    });
    let outcome = follow_log(&fixture.config_path, "web", 3, &|_line| {}).await;
    writer.await.expect("join the writer");
    assert!(outcome.is_err(), "got: {outcome:?}");
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn following_a_log_without_a_config_fails() {
    let outcome = follow_log("/nonexistent/pm3.yaml", "web", 1, &|_line| {}).await;
    assert!(outcome.is_err(), "got: {outcome:?}");
}

#[tokio::test]
async fn reading_a_log_without_a_config_fails() {
    assert!(
        read_log_tail("/nonexistent/pm3.yaml", "web", 5)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn listing_without_a_config_fails() {
    assert!(list_apps("/nonexistent/pm3.yaml").await.is_err());
}

#[tokio::test]
async fn starting_a_missing_apps_file_fails_before_any_daemon_call() {
    let outcome = start_apps("/nonexistent/pm3.yaml", "/nonexistent/apps.yaml").await;
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
