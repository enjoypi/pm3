use std::{fmt::Write as _, path::Path};

use tempfile::TempDir;

use super::*;

const THREE_LINES: &str = "first\nsecond\nthird\n";
const MAX_READ: u64 = 4_194_304;
const MAX_PENDING: u64 = 4_194_304;

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
    let lines = read_tail(&path, 2, MAX_READ).await.expect("should read");
    assert_eq!(lines, vec!["second", "third"]);
}

#[tokio::test]
async fn read_tail_reports_nothing_for_an_empty_log() {
    let (_dir, path) = temp_log(b"");
    assert!(
        read_tail(&path, 5, MAX_READ)
            .await
            .expect("should read")
            .is_empty()
    );
}

#[tokio::test]
async fn read_tail_reports_a_missing_log() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let err = read_tail(&dir.path().join("absent.log"), 5, MAX_READ)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot read log file"), "got: {err}");
}

#[tokio::test]
async fn following_skips_the_content_written_before_it_started() {
    let (_dir, path) = temp_log(THREE_LINES.as_bytes());
    let mut follower = LogFollower::start_at_end(&path, MAX_PENDING)
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
    let mut follower = LogFollower::start_at_end(&path, MAX_PENDING)
        .await
        .expect("should start");
    append(&path, b"fourth\n").await;
    let lines = follower.poll_appended().await.expect("should poll");
    assert_eq!(lines, vec!["fourth"]);
}

#[tokio::test]
async fn following_reports_appended_lines_in_order() {
    let (_dir, path) = temp_log(b"");
    let mut follower = LogFollower::start_at_end(&path, MAX_PENDING)
        .await
        .expect("should start");
    append(&path, b"one\ntwo\n").await;
    let lines = follower.poll_appended().await.expect("should poll");
    assert_eq!(lines, vec!["one", "two"]);
}

#[tokio::test]
async fn following_withholds_a_line_without_its_newline() {
    let (_dir, path) = temp_log(b"");
    let mut follower = LogFollower::start_at_end(&path, MAX_PENDING)
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
    let mut follower = LogFollower::start_at_end(&path, MAX_PENDING)
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
    let mut follower = LogFollower::start_at_end(&path, MAX_PENDING)
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
    let err = LogFollower::start_at_end(&dir.path().join("absent.log"), MAX_PENDING)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot read log file"), "got: {err}");
}

#[tokio::test]
async fn read_tail_reports_a_log_path_it_cannot_read() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let err = read_tail(dir.path(), 5, MAX_READ)
        .await
        .unwrap_err()
        .to_string();
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
    let tail = read_tail(&path, 2, MAX_READ).await.expect("should read");
    assert_eq!(tail, vec!["line 19998", "line 19999"]);
}

#[tokio::test]
async fn read_tail_reads_only_within_the_byte_budget() {
    let (_dir, path) = temp_log(b"xx\nyy\nzz\n");
    let tail = read_tail(&path, 10, 4).await.expect("should read");
    assert_eq!(tail, vec!["zz"]);
}

#[tokio::test]
async fn read_tail_bounds_a_line_that_never_ends() {
    let body = vec![b'x'; 200_000];
    let (_dir, path) = temp_log(&body);
    let tail = read_tail(&path, 10, 1024).await.expect("should read");
    assert_eq!(tail, vec!["x".repeat(1024)]);
}

#[tokio::test]
async fn following_releases_an_overgrown_partial_line_instead_of_hoarding_it() {
    let (_dir, path) = temp_log(b"");
    let mut follower = LogFollower::start_at_end(&path, 4)
        .await
        .expect("should start");
    append(&path, b"abcdefgh").await;
    follower.poll_appended().await.expect("first poll");
    let overflow = follower.poll_appended().await.expect("second poll");
    assert_eq!(overflow, vec!["abcdefgh"]);
    append(&path, b"ij\n").await;
    let lines = follower.poll_appended().await.expect("should poll");
    assert_eq!(lines, vec!["ij"]);
}

#[tokio::test]
async fn read_tail_reads_across_chunk_boundaries_when_it_must() {
    let body = long_log();
    let (_dir, path) = temp_log(body.as_bytes());
    let tail = read_tail(&path, 20_000, MAX_READ)
        .await
        .expect("should read");
    assert_eq!(tail.len(), 20_000);
    assert_eq!(tail.first().map(String::as_str), Some("line 0"));
}

#[tokio::test]
async fn following_replaces_undecodable_appended_bytes_instead_of_giving_up() {
    let (_dir, path) = temp_log(b"");
    let mut follower = LogFollower::start_at_end(&path, MAX_PENDING)
        .await
        .expect("should start");
    append(&path, &[0xff, 0xfe, b'\n']).await;
    let lines = follower
        .poll_appended()
        .await
        .expect("a binary byte must not abort the follow");
    assert_eq!(lines, vec!["\u{fffd}\u{fffd}".to_string()]);
}

#[tokio::test]
async fn following_keeps_a_multi_byte_character_split_across_two_polls() {
    let (_dir, path) = temp_log(b"");
    let mut follower = LogFollower::start_at_end(&path, MAX_PENDING)
        .await
        .expect("should start");
    let hanzi = "中".as_bytes();
    append(&path, &hanzi[..1]).await;
    assert!(
        follower
            .poll_appended()
            .await
            .expect("a partial character must not abort the follow")
            .is_empty()
    );
    append(&path, &[hanzi[1], hanzi[2], b'\n']).await;
    let lines = follower.poll_appended().await.expect("should read");
    assert_eq!(lines, vec!["中".to_string()]);
}

#[tokio::test]
async fn following_reports_a_log_it_cannot_read() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut follower = LogFollower::start_at_end(dir.path(), MAX_PENDING)
        .await
        .expect("a directory opens like a file on linux");
    let err = follower.poll_appended().await.unwrap_err().to_string();
    assert!(err.contains("cannot read log file"), "got: {err}");
}

#[tokio::test]
async fn following_rereads_a_truncated_log_from_the_start() {
    let (_dir, path) = temp_log(THREE_LINES.as_bytes());
    let mut follower = LogFollower::start_at_end(&path, MAX_PENDING)
        .await
        .expect("should start");
    std::fs::write(&path, b"fresh\n").expect("truncate the log");
    let lines = follower.poll_appended().await.expect("should poll");
    assert_eq!(lines, vec!["fresh"]);
}

#[tokio::test]
async fn following_drops_a_partial_line_when_the_log_is_truncated() {
    let (_dir, path) = temp_log(b"");
    let mut follower = LogFollower::start_at_end(&path, MAX_PENDING)
        .await
        .expect("should start");
    append(&path, b"halfway").await;
    follower.poll_appended().await.expect("first poll");
    std::fs::write(&path, b"new\n").expect("truncate the log");
    let lines = follower.poll_appended().await.expect("second poll");
    assert_eq!(lines, vec!["new"]);
}

#[cfg(unix)]
#[tokio::test]
async fn following_switches_to_the_new_file_after_rotation() {
    let (_dir, path) = temp_log(THREE_LINES.as_bytes());
    let mut follower = LogFollower::start_at_end(&path, MAX_PENDING)
        .await
        .expect("should start");
    std::fs::rename(&path, path.with_extension("1")).expect("rotate the log");
    std::fs::write(&path, b"rotated\n").expect("recreate the log");
    let lines = follower.poll_appended().await.expect("should poll");
    assert_eq!(lines, vec!["rotated"]);
}

#[cfg(unix)]
#[tokio::test]
async fn following_keeps_waiting_when_the_log_is_renamed_away() {
    let (_dir, path) = temp_log(THREE_LINES.as_bytes());
    let mut follower = LogFollower::start_at_end(&path, MAX_PENDING)
        .await
        .expect("should start");
    std::fs::rename(&path, path.with_extension("1")).expect("rename the log away");
    assert!(
        follower
            .poll_appended()
            .await
            .expect("a missing path must not abort the follow")
            .is_empty()
    );
}

#[tokio::test]
async fn a_conditional_follow_reports_a_missing_log_as_absent() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let missing = dir.path().join("ghost-out.log");
    let opened = LogFollower::start_at_end_if_exists(&missing, MAX_PENDING)
        .await
        .expect("a missing log is not an error");
    assert!(opened.is_none());
}

#[tokio::test]
async fn a_conditional_follow_opens_an_existing_log() {
    let (_dir, path) = temp_log(THREE_LINES.as_bytes());
    let opened = LogFollower::start_at_end_if_exists(&path, MAX_PENDING)
        .await
        .expect("should open");
    assert!(opened.is_some());
}

#[tokio::test]
async fn a_conditional_follow_still_fails_when_the_log_cannot_be_read() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, b"file").expect("write the blocker");
    let outcome =
        LogFollower::start_at_end_if_exists(&blocker.join("web-out.log"), MAX_PENDING).await;
    assert!(outcome.is_err(), "got: {outcome:?}");
}

#[tokio::test]
async fn a_strict_follow_still_reports_a_missing_log_as_an_error() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let missing = dir.path().join("ghost-out.log");
    let err = LogFollower::start_at_end(&missing, MAX_PENDING)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot read log file"), "got: {err}");
}

#[tokio::test]
async fn a_strict_follow_fails_when_the_log_cannot_be_read() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, b"file").expect("write the blocker");
    let outcome = LogFollower::start_at_end(&blocker.join("web-out.log"), MAX_PENDING).await;
    assert!(outcome.is_err(), "got: {outcome:?}");
}

#[tokio::test]
async fn following_reads_no_more_than_the_budget_per_poll() {
    let (_dir, path) = temp_log(b"");
    let mut follower = LogFollower::start_at_end(&path, 4)
        .await
        .expect("should start");
    append(&path, b"abcd\nefgh\nij\n").await;
    let mut seen: Vec<String> = Vec::new();
    let mut polls = 0;
    while seen.len() < 3 && polls < 20 {
        seen.extend(follower.poll_appended().await.expect("should poll"));
        polls += 1;
    }
    assert_eq!(seen, vec!["abcd", "efgh", "ij"], "no line may be lost");
    assert!(
        polls > 1,
        "a four byte budget must not swallow thirteen bytes in one read"
    );
}
