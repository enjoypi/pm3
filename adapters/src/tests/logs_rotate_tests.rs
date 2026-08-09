#![cfg(unix)]
use super::*;

fn logs_dir_with(files: &[(&str, usize)]) -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().expect("temp dir");
    for (name, size) in files {
        std::fs::write(dir.path().join(name), vec![b'x'; *size]).expect("seed the log");
    }
    let logs_dir = dir.path().to_string_lossy().into_owned();
    (dir, logs_dir)
}

async fn rotate(dir: &std::path::Path, max_bytes: u64) -> Vec<RotatedLog> {
    CopyTruncateRotator
        .rotate_logs(&dir.to_string_lossy(), max_bytes)
        .await
        .expect("should rotate")
}

#[tokio::test]
async fn an_oversized_log_is_copied_aside_and_truncated() {
    let (dir, logs_dir) = logs_dir_with(&[("web-out.log", 4096)]);
    let rotated = CopyTruncateRotator
        .rotate_logs(&logs_dir, 1024)
        .await
        .expect("should rotate");
    assert_eq!(rotated.len(), 1);
    assert_eq!(rotated[0].bytes, 4096);
    let original = dir.path().join("web-out.log");
    assert_eq!(std::fs::metadata(&original).expect("stat").len(), 0);
    let backup = dir.path().join("web-out.log.1");
    assert_eq!(std::fs::metadata(&backup).expect("stat backup").len(), 4096);
}

#[tokio::test]
async fn a_log_inside_the_limit_is_left_alone() {
    let (dir, logs_dir) = logs_dir_with(&[("web-out.log", 512)]);
    let rotated = CopyTruncateRotator
        .rotate_logs(&logs_dir, 1024)
        .await
        .expect("should rotate");
    assert!(rotated.is_empty());
    assert_eq!(
        std::fs::metadata(dir.path().join("web-out.log"))
            .expect("stat")
            .len(),
        512
    );
}

#[tokio::test]
async fn the_backup_keeps_a_single_generation() {
    let (dir, logs_dir) = logs_dir_with(&[("web-out.log", 4096)]);
    std::fs::write(dir.path().join("web-out.log.1"), vec![b'y'; 8]).expect("seed the old backup");
    let rotated = CopyTruncateRotator
        .rotate_logs(&logs_dir, 1024)
        .await
        .expect("should rotate");
    assert_eq!(rotated.len(), 1);
    let backup = std::fs::read(dir.path().join("web-out.log.1")).expect("read the backup");
    assert_eq!(backup, vec![b'x'; 4096]);
}

#[tokio::test]
async fn files_that_are_not_service_logs_are_not_touched() {
    let (_dir, logs_dir) = logs_dir_with(&[("web-out.log.1", 4096), ("notes.txt", 4096)]);
    let rotated = CopyTruncateRotator
        .rotate_logs(&logs_dir, 1024)
        .await
        .expect("should rotate");
    assert!(rotated.is_empty());
}

#[tokio::test]
async fn the_err_log_rotates_with_the_same_rule() {
    let (_dir, logs_dir) = logs_dir_with(&[("web-err.log", 4096)]);
    let rotated = CopyTruncateRotator
        .rotate_logs(&logs_dir, 1024)
        .await
        .expect("should rotate");
    assert_eq!(rotated.len(), 1);
    assert!(rotated[0].path.ends_with("web-err.log"));
}

#[cfg(unix)]
#[tokio::test]
async fn a_non_utf8_file_name_is_skipped() {
    use std::os::unix::ffi::OsStrExt as _;

    let dir = tempfile::tempdir().expect("temp dir");
    let weird = std::ffi::OsStr::from_bytes(b"bad-\xff-out.log");
    std::fs::write(dir.path().join(weird), vec![b'x'; 4096]).expect("seed the log");
    let rotated = rotate(dir.path(), 1024).await;
    assert!(rotated.is_empty());
}

#[tokio::test]
async fn a_missing_logs_directory_is_a_scan_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let missing = dir.path().join("nope").to_string_lossy().into_owned();
    let err = CopyTruncateRotator
        .rotate_logs(&missing, 1024)
        .await
        .unwrap_err();
    assert!(matches!(err, LogRotateError::Scan { .. }), "got: {err}");
}

#[tokio::test]
async fn one_broken_log_does_not_stop_the_others() {
    let (dir, logs_dir) = logs_dir_with(&[("web-out.log", 4096), ("api-out.log", 4096)]);
    let blocked = dir.path().join("web-out.log.1");
    std::fs::create_dir(&blocked).expect("block the backup path");
    let rotated = CopyTruncateRotator
        .rotate_logs(&logs_dir, 1024)
        .await
        .expect("one broken log must not fail the batch");
    assert_eq!(rotated.len(), 1);
    assert!(rotated[0].path.ends_with("api-out.log"));
}

#[cfg(unix)]
#[tokio::test]
async fn a_dangling_symlink_where_a_log_should_be_is_skipped() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::os::unix::fs::symlink("missing-target", dir.path().join("web-out.log"))
        .expect("plant a dangling symlink");
    std::fs::write(dir.path().join("api-out.log"), vec![b'x'; 4096]).expect("seed the log");
    let rotated = rotate(dir.path(), 1024).await;
    assert_eq!(rotated.len(), 1);
    assert!(rotated[0].path.ends_with("api-out.log"));
}

#[cfg(unix)]
#[tokio::test]
async fn a_readonly_log_cannot_be_truncated_and_is_skipped() {
    use std::os::unix::fs::PermissionsExt as _;

    let (dir, _logs_dir) = logs_dir_with(&[("web-out.log", 4096), ("api-out.log", 4096)]);
    std::fs::set_permissions(
        dir.path().join("web-out.log"),
        std::fs::Permissions::from_mode(0o444),
    )
    .expect("make the log readonly");
    let rotated = rotate(dir.path(), 1024).await;
    assert_eq!(rotated.len(), 1);
    assert!(rotated[0].path.ends_with("api-out.log"));
    assert_eq!(
        std::fs::metadata(dir.path().join("web-out.log"))
            .expect("stat")
            .len(),
        4096
    );
}
