use super::*;

#[tokio::test]
async fn reading_logs_returns_the_tail() {
    let fixture = crate::daemon_fixture::running_daemon().await;
    crate::daemon_fixture::seed_log(&fixture, "web", "first\nsecond\n");
    let printed = execute(parse(&[
        "pm3",
        "--config",
        &fixture.config_path,
        "logs",
        "web",
        "-n",
        "1",
    ]))
    .await
    .expect("should read the log");
    assert_eq!(printed.as_deref(), Some("second"));
    crate::daemon_fixture::stop_daemon(fixture).await;
}

#[tokio::test]
async fn following_logs_prints_and_returns_nothing() {
    let fixture = crate::daemon_fixture::running_daemon().await;
    crate::daemon_fixture::seed_log(&fixture, "web", "first\n");
    let printed = run_logs(&fixture.config_path, "web", 1, true, 1)
        .await
        .expect("should follow the log");
    assert_eq!(printed, None);
    crate::daemon_fixture::stop_daemon(fixture).await;
}

#[tokio::test]
async fn following_a_missing_log_fails_the_command() {
    let fixture = crate::daemon_fixture::running_daemon().await;
    let outcome = run_logs(&fixture.config_path, "ghost", 1, true, 1).await;
    assert!(outcome.is_err(), "got: {outcome:?}");
    crate::daemon_fixture::stop_daemon(fixture).await;
}

#[tokio::test]
async fn following_a_log_that_turns_undecodable_keeps_going() {
    let fixture = crate::daemon_fixture::running_daemon().await;
    let path = crate::daemon_fixture::seed_log(&fixture, "web", "old\n");
    let writer = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(60)).await;
        std::fs::write(&path, [b'o', b'l', b'd', b'\n', 0xff, 0xfe, b'\n']).expect("append");
    });
    let outcome = run_logs(&fixture.config_path, "web", 1, true, 3).await;
    writer.await.expect("join the writer");
    assert!(outcome.is_ok(), "got: {outcome:?}");
    crate::daemon_fixture::stop_daemon(fixture).await;
}

#[tokio::test]
async fn reading_the_log_of_an_unsafe_name_is_refused_before_touching_the_disk() {
    let fixture = crate::daemon_fixture::running_daemon().await;
    let outcome = run_logs(&fixture.config_path, "../../etc/passwd", 1, false, 1).await;
    assert!(outcome.is_err(), "got: {outcome:?}");
    crate::daemon_fixture::stop_daemon(fixture).await;
}
