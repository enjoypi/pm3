use std::{fmt::Write as _, path::Path};

use tempfile::TempDir;

use super::*;

const THREE_LINES: &str = "first\nsecond\nthird\n";

fn temp_log(content: &[u8]) -> (TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("web-out.log");
    std::fs::write(&path, content).expect("seed the log");
    (dir, path)
}

async fn append(path: &Path, content: &[u8]) {
    let mut file = tokio::fs::OpenOptions::new()
        .append(true)
        .open(path)
        .await
        .expect("open for append");
    tokio::io::AsyncWriteExt::write_all(&mut file, content)
        .await
        .expect("append");
    tokio::io::AsyncWriteExt::flush(&mut file)
        .await
        .expect("flush");
}

#[test]
fn tail_lines_reports_nothing_for_empty_content() {
    assert!(tail_lines("", 10).is_empty());
}

#[test]
fn tail_lines_reports_every_line_when_asked_for_more() {
    assert_eq!(
        tail_lines(THREE_LINES, 10),
        vec!["first", "second", "third"]
    );
}

#[test]
fn tail_lines_reports_only_the_last_lines() {
    assert_eq!(tail_lines(THREE_LINES, 2), vec!["second", "third"]);
}

#[test]
fn tail_lines_reports_nothing_when_asked_for_none() {
    assert!(tail_lines(THREE_LINES, 0).is_empty());
}

#[test]
fn tail_lines_ignores_the_trailing_newline() {
    assert_eq!(tail_lines("only\n", 5), vec!["only"]);
}

#[test]
fn tail_lines_keeps_an_unterminated_last_line() {
    assert_eq!(tail_lines("first\npartial", 5), vec!["first", "partial"]);
}

#[tokio::test]
async fn read_tail_reports_the_last_lines_of_a_log() {
    let (_dir, path) = temp_log(THREE_LINES.as_bytes());
    let lines = read_tail(&path, 2).await.expect("should read");
    assert_eq!(lines, vec!["second", "third"]);
}

#[tokio::test]
async fn read_tail_reports_nothing_for_an_empty_log() {
    let (_dir, path) = temp_log(b"");
    assert!(read_tail(&path, 5).await.expect("should read").is_empty());
}

#[tokio::test]
async fn read_tail_reports_a_missing_log() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let err = read_tail(&dir.path().join("absent.log"), 5)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot read log file"), "got: {err}");
}

#[tokio::test]
async fn following_skips_the_content_written_before_it_started() {
    let (_dir, path) = temp_log(THREE_LINES.as_bytes());
    let mut follower = LogFollower::start_at_end(&path)
        .await
        .expect("should start");
    assert!(
        follower
            .poll_appended()
            .await
            .expect("should poll")
            .is_empty()
    );
}

#[tokio::test]
async fn following_reports_an_appended_line() {
    let (_dir, path) = temp_log(THREE_LINES.as_bytes());
    let mut follower = LogFollower::start_at_end(&path)
        .await
        .expect("should start");
    append(&path, b"fourth\n").await;
    let lines = follower.poll_appended().await.expect("should poll");
    assert_eq!(lines, vec!["fourth"]);
}

#[tokio::test]
async fn following_reports_appended_lines_in_order() {
    let (_dir, path) = temp_log(b"");
    let mut follower = LogFollower::start_at_end(&path)
        .await
        .expect("should start");
    append(&path, b"one\ntwo\n").await;
    let lines = follower.poll_appended().await.expect("should poll");
    assert_eq!(lines, vec!["one", "two"]);
}

#[tokio::test]
async fn following_withholds_a_line_without_its_newline() {
    let (_dir, path) = temp_log(b"");
    let mut follower = LogFollower::start_at_end(&path)
        .await
        .expect("should start");
    append(&path, b"halfway").await;
    assert!(
        follower
            .poll_appended()
            .await
            .expect("should poll")
            .is_empty()
    );
}

#[tokio::test]
async fn following_releases_a_withheld_line_once_it_ends() {
    let (_dir, path) = temp_log(b"");
    let mut follower = LogFollower::start_at_end(&path)
        .await
        .expect("should start");
    append(&path, b"halfway").await;
    follower.poll_appended().await.expect("first poll");
    append(&path, b" there\n").await;
    let lines = follower.poll_appended().await.expect("second poll");
    assert_eq!(lines, vec!["halfway there"]);
}

#[tokio::test]
async fn following_reports_nothing_when_the_log_has_not_grown() {
    let (_dir, path) = temp_log(b"");
    let mut follower = LogFollower::start_at_end(&path)
        .await
        .expect("should start");
    append(&path, b"one\n").await;
    follower.poll_appended().await.expect("first poll");
    assert!(
        follower
            .poll_appended()
            .await
            .expect("second poll")
            .is_empty()
    );
}

#[tokio::test]
async fn following_reports_a_missing_log() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let err = LogFollower::start_at_end(&dir.path().join("absent.log"))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot read log file"), "got: {err}");
}

#[tokio::test]
async fn read_tail_reports_a_log_path_it_cannot_read() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let err = read_tail(dir.path(), 5).await.unwrap_err().to_string();
    assert!(err.contains("cannot read log file"), "got: {err}");
}

fn long_log() -> String {
    (0..20_000).fold(String::new(), |mut text, line| {
        let _ = writeln!(text, "line {line}");
        text
    })
}

#[tokio::test]
async fn read_tail_reads_only_the_tail_of_a_long_log() {
    let body = long_log();
    let (_dir, path) = temp_log(body.as_bytes());
    let tail = read_tail(&path, 2).await.expect("should read");
    assert_eq!(tail, vec!["line 19998", "line 19999"]);
}

#[tokio::test]
async fn read_tail_reads_across_chunk_boundaries_when_it_must() {
    let body = long_log();
    let (_dir, path) = temp_log(body.as_bytes());
    let tail = read_tail(&path, 20_000).await.expect("should read");
    assert_eq!(tail.len(), 20_000);
    assert_eq!(tail.first().map(String::as_str), Some("line 0"));
}

#[tokio::test]
async fn following_reports_undecodable_appended_content() {
    let (_dir, path) = temp_log(b"");
    let mut follower = LogFollower::start_at_end(&path)
        .await
        .expect("should start");
    append(&path, &[0xff, 0xfe, b'\n']).await;
    let err = follower.poll_appended().await.unwrap_err().to_string();
    assert!(err.contains("cannot read log file"), "got: {err}");
}
