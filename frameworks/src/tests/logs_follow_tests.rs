#![cfg(unix)]
use super::*;

#[tokio::test]
async fn following_an_aggregation_skips_a_service_without_a_log() {
    let fixture = running_daemon().await;
    declare(&fixture, &["web", "api"]);
    let web = seed_log(&fixture, "web", LogStream::Stdout, "web-old\n");
    let collected = Collected::default();
    let writer = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(60)).await;
        std::fs::write(&web, "web-old\nweb-new\n").expect("append web");
    });
    let request = LogRequest {
        lines: Some(1),
        follow: true,
        polls: 2,
        ..reading(&[])
    };
    let outcome = run_logs(&fixture.config_path, &request, &|line| collected.push(line)).await;
    writer.await.expect("join the writer");
    assert!(outcome.is_ok(), "got: {outcome:?}");
    assert_eq!(
        collected.taken(),
        vec!["web | web-old".to_string(), "web | web-new".to_string()]
    );
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn following_the_log_of_an_unsafe_name_is_refused() {
    let fixture = running_daemon().await;
    let request = LogRequest {
        follow: true,
        polls: 1,
        ..reading(&["../escape"])
    };
    let outcome = run_logs(&fixture.config_path, &request, &|_| {}).await;
    assert!(outcome.is_err(), "got: {outcome:?}");
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn reading_the_log_tail_of_an_unsafe_name_is_refused() {
    let fixture = running_daemon().await;
    let outcome = run_logs(&fixture.config_path, &reading(&["../escape"]), &|_| {}).await;
    assert!(outcome.is_err(), "got: {outcome:?}");
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn following_a_log_without_a_config_fails() {
    let request = LogRequest {
        follow: true,
        polls: 1,
        ..reading(&["web"])
    };
    let outcome = run_logs("/nonexistent/pm3.yaml", &request, &|_| {}).await;
    assert!(outcome.is_err(), "got: {outcome:?}");
}

#[tokio::test]
async fn reading_a_log_without_a_config_fails() {
    let outcome = run_logs("/nonexistent/pm3.yaml", &reading(&["web"]), &|_| {}).await;
    assert!(outcome.is_err(), "got: {outcome:?}");
}

#[tokio::test]
async fn following_a_log_path_that_is_not_a_file_fails() {
    let fixture = running_daemon().await;
    let blocked = log_path(
        &fixture.paths.logs_dir.to_string_lossy(),
        "blocked",
        LogStream::Stdout,
    );
    std::fs::create_dir_all(&blocked).expect("block the log path with a directory");
    let request = LogRequest {
        follow: true,
        polls: 1,
        ..reading(&["blocked"])
    };
    let outcome = run_logs(&fixture.config_path, &request, &|_| {}).await;
    assert!(outcome.is_err(), "got: {outcome:?}");
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn opening_a_follower_fails_for_a_missing_log_in_strict_mode() {
    let fixture = running_daemon().await;
    let targets = vec![LogTarget {
        name: "ghost".to_string(),
        stream: LogStream::Stdout,
        path: log_path(
            &fixture.paths.logs_dir.to_string_lossy(),
            "ghost",
            LogStream::Stdout,
        ),
        prefix: String::new(),
    }];
    let outcome = open_followers(&targets, true, 4_194_304).await;
    assert!(outcome.is_err(), "a missing log must fail in strict mode");
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn opening_a_follower_fails_for_an_unreadable_log_in_lenient_mode() {
    let fixture = running_daemon().await;
    let blocker = fixture.dir.path().join("blocker");
    std::fs::write(&blocker, b"file").expect("write the blocker");
    let targets = vec![LogTarget {
        name: "ghost".to_string(),
        stream: LogStream::Stdout,
        path: blocker.join("ghost-out.log").to_string_lossy().into_owned(),
        prefix: "ghost | ".to_string(),
    }];
    let outcome = open_followers(&targets, false, 4_194_304).await;
    assert!(
        outcome.is_err(),
        "an unreadable log must fail even in lenient mode"
    );
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn follow_targets_propagates_the_open_failure() {
    let fixture = running_daemon().await;
    let session = open_session(&fixture.config_path).expect("open the session");
    let targets = vec![LogTarget {
        name: "ghost".to_string(),
        stream: LogStream::Stdout,
        path: log_path(
            &fixture.paths.logs_dir.to_string_lossy(),
            "ghost",
            LogStream::Stdout,
        ),
        prefix: String::new(),
    }];
    let outcome = follow_targets(&session, &targets, true, 1, &|_| {}).await;
    assert!(outcome.is_err(), "a missing log must fail the follow");
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn following_fails_when_the_log_turns_into_a_directory_mid_follow() {
    let fixture = running_daemon().await;
    let path = seed_log(&fixture, "web", LogStream::Stdout, "old\n");
    let writer = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(60)).await;
        let swapped = std::path::PathBuf::from(&path);
        std::fs::remove_file(&swapped).expect("remove the log");
        std::fs::create_dir(&swapped).expect("swap in a directory");
    });
    let request = LogRequest {
        lines: Some(1),
        follow: true,
        polls: 2,
        ..reading(&["web"])
    };
    let outcome = run_logs(&fixture.config_path, &request, &|_| {}).await;
    writer.await.expect("join the writer");
    assert!(outcome.is_err(), "a vanished log must fail the follow");
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn reading_a_log_for_an_unsafe_name_fails() {
    let fixture = running_daemon().await;
    let outcome = run_logs(&fixture.config_path, &reading(&["../escape"]), &|_| {}).await;
    assert!(outcome.is_err(), "got: {outcome:?}");
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn the_cli_reads_the_stderr_log_with_the_err_flag() {
    let fixture = running_daemon().await;
    seed_log(&fixture, "web", LogStream::Stderr, "boom\n");
    let printed = execute(Cli::parse_from([
        "pm3",
        "--config",
        &fixture.config_path,
        "logs",
        "web",
        "--err",
    ]))
    .await
    .expect("should read the err log");
    assert_eq!(printed.as_deref(), Some("boom"));
    stop_daemon(fixture).await;
}

fn clearing(items: &[&str]) -> LogRequest {
    LogRequest {
        action: LogAction::Clear,
        ..reading(items)
    }
}

#[tokio::test]
async fn clearing_a_log_truncates_it_and_reports_the_path() {
    let fixture = running_daemon().await;
    let path = seed_log(&fixture, "web", LogStream::Stdout, "old\n");
    let printed = run_logs(&fixture.config_path, &clearing(&["web"]), &|_| {})
        .await
        .expect("should clear");
    assert_eq!(printed.as_deref(), Some(format!("cleared {path}").as_str()));
    assert_eq!(std::fs::metadata(&path).expect("stat").len(), 0);
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn clearing_with_err_only_truncates_the_stderr_log() {
    let fixture = running_daemon().await;
    let out = seed_log(&fixture, "web", LogStream::Stdout, "out\n");
    let err = seed_log(&fixture, "web", LogStream::Stderr, "err\n");
    let request = LogRequest {
        err: true,
        ..clearing(&["web"])
    };
    let printed = run_logs(&fixture.config_path, &request, &|_| {})
        .await
        .expect("should clear");
    assert_eq!(printed.as_deref(), Some(format!("cleared {err}").as_str()));
    assert_eq!(std::fs::metadata(&err).expect("stat err").len(), 0);
    assert_eq!(std::fs::metadata(&out).expect("stat out").len(), 4);
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn clearing_with_all_truncates_both_streams() {
    let fixture = running_daemon().await;
    let out = seed_log(&fixture, "web", LogStream::Stdout, "out\n");
    let err = seed_log(&fixture, "web", LogStream::Stderr, "err\n");
    let request = LogRequest {
        all: true,
        ..clearing(&["web"])
    };
    let printed = run_logs(&fixture.config_path, &request, &|_| {})
        .await
        .expect("should clear");
    assert_eq!(
        printed.as_deref(),
        Some(format!("cleared {out}\ncleared {err}").as_str())
    );
    assert_eq!(std::fs::metadata(&out).expect("stat out").len(), 0);
    assert_eq!(std::fs::metadata(&err).expect("stat err").len(), 0);
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn clearing_a_missing_log_fails() {
    let fixture = running_daemon().await;
    let err = run_logs(&fixture.config_path, &clearing(&["ghost"]), &|_| {})
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot clear log file"), "got: {err}");
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn clearing_an_aggregation_truncates_every_declared_service_and_skips_missing_logs() {
    let fixture = running_daemon().await;
    declare(&fixture, &["web", "api"]);
    let path = seed_log(&fixture, "web", LogStream::Stdout, "web-line\n");
    let printed = run_logs(&fixture.config_path, &clearing(&[]), &|_| {})
        .await
        .expect("should clear");
    assert_eq!(printed.as_deref(), Some(format!("cleared {path}").as_str()));
    assert_eq!(std::fs::metadata(&path).expect("stat").len(), 0);
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn clearing_the_log_of_an_unsafe_name_is_refused() {
    let fixture = running_daemon().await;
    let outcome = run_logs(&fixture.config_path, &clearing(&["../escape"]), &|_| {}).await;
    assert!(outcome.is_err(), "got: {outcome:?}");
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn the_cli_clears_a_log_with_the_clear_flag() {
    let fixture = running_daemon().await;
    let path = seed_log(&fixture, "web", LogStream::Stdout, "old\n");
    let printed = execute(Cli::parse_from([
        "pm3",
        "--config",
        &fixture.config_path,
        "logs",
        "web",
        "--clear",
    ]))
    .await
    .expect("should clear");
    assert_eq!(printed.as_deref(), Some(format!("cleared {path}").as_str()));
    assert_eq!(std::fs::metadata(&path).expect("stat").len(), 0);
    stop_daemon(fixture).await;
}

#[test]
fn the_clear_flag_conflicts_with_follow_and_a_line_count() {
    assert!(
        Cli::try_parse_from(["pm3", "logs", "web", "--clear", "--follow"]).is_err(),
        "--clear must conflict with --follow"
    );
    assert!(
        Cli::try_parse_from(["pm3", "logs", "web", "--clear", "-n", "5"]).is_err(),
        "--clear must conflict with -n"
    );
}
