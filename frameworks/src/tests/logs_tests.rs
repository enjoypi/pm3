use std::fmt::Write as _;

use clap::Parser as _;

use super::*;
use crate::{
    cli::{Cli, execute},
    daemon_fixture::{Collected, Fixture, running_daemon, seed_log, stop_daemon},
    test_support::LOG_TAIL_LINES,
};

fn names(items: &[&str]) -> Vec<String> {
    items.iter().map(ToString::to_string).collect()
}

fn reading(items: &[&str]) -> LogRequest {
    LogRequest {
        names: names(items),
        ..LogRequest::default()
    }
}

fn declare(fixture: &Fixture, declared: &[&str]) {
    let cfg_dir = fixture.paths.root.join("service");
    std::fs::create_dir_all(&cfg_dir).expect("create the service directory");
    for name in declared {
        std::fs::write(cfg_dir.join(format!("{name}.yaml")), "").expect("declare the service");
    }
}

#[tokio::test]
async fn reading_a_log_tail_without_a_count_falls_back_to_the_configured_default() {
    let fixture = running_daemon().await;
    let configured = usize::try_from(LOG_TAIL_LINES).expect("the fixture count fits usize");
    let total = configured + 5;
    let body = (1..=total).fold(String::new(), |mut text, n| {
        let _ = writeln!(text, "line-{n}");
        text
    });
    seed_log(&fixture, "web", LogStream::Stdout, &body);
    let printed = run_logs(&fixture.config_path, &reading(&["web"]), &|_| {})
        .await
        .expect("should read");
    let expected = ((total - configured + 1)..=total)
        .map(|n| format!("line-{n}"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(printed.as_deref(), Some(expected.as_str()));
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn reading_a_log_tail_returns_the_last_lines() {
    let fixture = running_daemon().await;
    seed_log(&fixture, "web", LogStream::Stdout, "first\nsecond\nthird\n");
    let request = LogRequest {
        lines: Some(2),
        ..reading(&["web"])
    };
    let printed = run_logs(&fixture.config_path, &request, &|_| {})
        .await
        .expect("should read");
    assert_eq!(printed.as_deref(), Some("second\nthird"));
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn reading_a_missing_log_fails() {
    let fixture = running_daemon().await;
    let err = run_logs(&fixture.config_path, &reading(&["ghost"]), &|_| {})
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot read log file"), "got: {err}");
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn reading_the_stderr_log_with_the_err_flag() {
    let fixture = running_daemon().await;
    seed_log(&fixture, "web", LogStream::Stderr, "boom\n");
    let request = LogRequest {
        err: true,
        ..reading(&["web"])
    };
    let printed = run_logs(&fixture.config_path, &request, &|_| {})
        .await
        .expect("should read");
    assert_eq!(printed.as_deref(), Some("boom"));
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn reading_both_streams_with_all_tags_each_line() {
    let fixture = running_daemon().await;
    seed_log(&fixture, "web", LogStream::Stdout, "out-line\n");
    seed_log(&fixture, "web", LogStream::Stderr, "err-line\n");
    let request = LogRequest {
        all: true,
        ..reading(&["web"])
    };
    let printed = run_logs(&fixture.config_path, &request, &|_| {})
        .await
        .expect("should read");
    assert_eq!(
        printed.as_deref(),
        Some("web [out] | out-line\nweb [err] | err-line")
    );
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn aggregating_without_names_reads_every_declared_service_in_order() {
    let fixture = running_daemon().await;
    declare(&fixture, &["web", "api"]);
    seed_log(&fixture, "web", LogStream::Stdout, "web-line\n");
    seed_log(&fixture, "api", LogStream::Stdout, "api-line\n");
    let printed = run_logs(&fixture.config_path, &reading(&[]), &|_| {})
        .await
        .expect("should read");
    assert_eq!(printed.as_deref(), Some("api | api-line\nweb | web-line"));
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn aggregating_named_services_prefixes_each_line() {
    let fixture = running_daemon().await;
    seed_log(&fixture, "web", LogStream::Stdout, "web-line\n");
    seed_log(&fixture, "api", LogStream::Stdout, "api-line\n");
    let printed = run_logs(&fixture.config_path, &reading(&["web", "api"]), &|_| {})
        .await
        .expect("should read");
    assert_eq!(printed.as_deref(), Some("web | web-line\napi | api-line"));
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn aggregation_skips_a_service_without_a_log() {
    let fixture = running_daemon().await;
    declare(&fixture, &["web", "api"]);
    seed_log(&fixture, "web", LogStream::Stdout, "web-line\n");
    let printed = run_logs(&fixture.config_path, &reading(&[]), &|_| {})
        .await
        .expect("should read");
    assert_eq!(printed.as_deref(), Some("web | web-line"));
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn aggregation_skips_a_service_whose_log_path_is_blocked() {
    let fixture = running_daemon().await;
    declare(&fixture, &["web", "api"]);
    seed_log(&fixture, "web", LogStream::Stdout, "web-line\n");
    let blocked = log_file(&fixture.paths, "api", LogStream::Stdout).expect("a safe name");
    std::fs::create_dir_all(&blocked).expect("block the log path with a directory");
    let printed = run_logs(&fixture.config_path, &reading(&[]), &|_| {})
        .await
        .expect("should read");
    assert_eq!(printed.as_deref(), Some("web | web-line"));
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn files_that_are_not_service_declarations_are_skipped() {
    let fixture = running_daemon().await;
    declare(&fixture, &["web"]);
    let cfg_dir = fixture.paths.root.join("service");
    std::fs::write(cfg_dir.join("web.env"), "TOKEN=x").expect("write an env sidecar");
    std::fs::write(cfg_dir.join("2.yaml"), "").expect("write a numeric name");
    std::fs::write(cfg_dir.join(".yaml"), "").expect("write an empty name");
    seed_log(&fixture, "web", LogStream::Stdout, "web-line\n");
    let printed = run_logs(&fixture.config_path, &reading(&[]), &|_| {})
        .await
        .expect("should read");
    assert_eq!(printed.as_deref(), Some("web-line"));
    stop_daemon(fixture).await;
}

#[cfg(unix)]
#[tokio::test]
async fn a_non_utf8_file_name_in_the_service_directory_is_skipped() {
    use std::os::unix::ffi::OsStrExt as _;

    let fixture = running_daemon().await;
    declare(&fixture, &["web"]);
    let cfg_dir = fixture.paths.root.join("service");
    let weird = std::ffi::OsStr::from_bytes(b"bad-\xff.yaml");
    std::fs::write(cfg_dir.join(weird), "").expect("write a non-UTF8 name");
    seed_log(&fixture, "web", LogStream::Stdout, "web-line\n");
    let printed = run_logs(&fixture.config_path, &reading(&[]), &|_| {})
        .await
        .expect("should read");
    assert_eq!(printed.as_deref(), Some("web-line"));
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn aggregating_without_a_service_directory_reads_nothing() {
    let dir = tempfile::tempdir().expect("temp dir");
    let home = dir.path().join("home");
    let config = crate::test_support::write_config(dir.path(), &home.to_string_lossy());
    let printed = run_logs(config.to_str().expect("path"), &reading(&[]), &|_| {})
        .await
        .expect("should read nothing");
    assert_eq!(printed.as_deref(), Some(""));
}

#[tokio::test]
async fn following_a_log_emits_the_tail_then_the_appended_lines() {
    let fixture = running_daemon().await;
    let path = seed_log(&fixture, "web", LogStream::Stdout, "old\n");
    let collected = Collected::default();
    let writer = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(60)).await;
        std::fs::write(&path, "old\nfresh\n").expect("append a line");
    });
    let request = LogRequest {
        lines: Some(1),
        follow: true,
        polls: 2,
        ..reading(&["web"])
    };
    run_logs(&fixture.config_path, &request, &|line| collected.push(line))
        .await
        .expect("should follow");
    writer.await.expect("join the writer");
    assert_eq!(collected.taken(), vec!["old", "fresh"]);
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn following_a_missing_log_fails() {
    let fixture = running_daemon().await;
    let request = LogRequest {
        follow: true,
        polls: 1,
        ..reading(&["ghost"])
    };
    let outcome = run_logs(&fixture.config_path, &request, &|_| {}).await;
    assert!(outcome.is_err(), "got: {outcome:?}");
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn following_a_log_that_turns_undecodable_keeps_reporting_lines() {
    let fixture = running_daemon().await;
    let path = seed_log(&fixture, "web", LogStream::Stdout, "old\n");
    let writer = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(60)).await;
        std::fs::write(&path, [b'o', b'l', b'd', b'\n', 0xff, 0xfe, b'\n'])
            .expect("append raw bytes");
    });
    let request = LogRequest {
        follow: true,
        polls: 3,
        ..reading(&["web"])
    };
    let outcome = run_logs(&fixture.config_path, &request, &|_| {}).await;
    writer.await.expect("join the writer");
    assert!(outcome.is_ok(), "got: {outcome:?}");
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn following_multiple_services_merges_their_appends() {
    let fixture = running_daemon().await;
    declare(&fixture, &["web", "api"]);
    let web = seed_log(&fixture, "web", LogStream::Stdout, "web-old\n");
    let api = seed_log(&fixture, "api", LogStream::Stdout, "api-old\n");
    let collected = Collected::default();
    let writer = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(60)).await;
        std::fs::write(&web, "web-old\nweb-new\n").expect("append web");
        std::fs::write(&api, "api-old\napi-new\n").expect("append api");
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
    let seen = collected.taken();
    assert!(seen.contains(&"api | api-new".to_string()), "got: {seen:?}");
    assert!(seen.contains(&"web | web-new".to_string()), "got: {seen:?}");
    stop_daemon(fixture).await;
}

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
    let blocked = log_file(&fixture.paths, "blocked", LogStream::Stdout).expect("a safe name");
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
        path: log_file(&fixture.paths, "ghost", LogStream::Stdout).expect("a safe name"),
        prefix: String::new(),
    }];
    let outcome = open_followers(&targets, true).await;
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
    let outcome = open_followers(&targets, false).await;
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
        path: log_file(&fixture.paths, "ghost", LogStream::Stdout).expect("a safe name"),
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
async fn log_file_rejects_an_unsafe_name() {
    let fixture = running_daemon().await;
    let outcome = log_file(&fixture.paths, "../escape", LogStream::Stdout);
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
