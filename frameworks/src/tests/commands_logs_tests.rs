use super::*;

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
async fn following_a_log_that_turns_undecodable_keeps_reporting_lines() {
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
    assert!(outcome.is_ok(), "got: {outcome:?}");
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn following_the_log_of_an_unsafe_name_is_refused() {
    let fixture = running_daemon().await;
    let outcome = follow_log(&fixture.config_path, "../escape", 1, &|_line| {}).await;
    assert!(outcome.is_err(), "got: {outcome:?}");
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn reading_the_log_tail_of_an_unsafe_name_is_refused() {
    let fixture = running_daemon().await;
    let outcome = read_log_tail(&fixture.config_path, "../escape", 1).await;
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
