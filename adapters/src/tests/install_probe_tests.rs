use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use super::*;

const PROBE_TIMEOUT_MS: u64 = 5000;
const SLOW_BINARY_TIMEOUT_MS: u64 = 50;

fn fake_binary(dir: &Path, unix: &str, windows: &str) -> PathBuf {
    crate::platform::script(dir, "old-pm3", unix, windows)
}

#[tokio::test]
async fn a_binary_that_prints_a_version_is_named_by_it() {
    let dir = tempfile::tempdir().expect("temp dir");
    let binary = fake_binary(dir.path(), "echo 'pm3 1.8.0'", "echo pm3 1.8.0");
    assert_eq!(
        binary_version(&binary, PROBE_TIMEOUT_MS).await,
        Some("1.8.0".to_string())
    );
}

#[tokio::test]
async fn a_missing_binary_has_no_version() {
    let dir = tempfile::tempdir().expect("temp dir");
    assert_eq!(
        binary_version(&dir.path().join("missing"), PROBE_TIMEOUT_MS).await,
        None
    );
}

#[tokio::test]
async fn a_failing_binary_has_no_version() {
    let dir = tempfile::tempdir().expect("temp dir");
    let binary = fake_binary(dir.path(), "exit 1", "exit /b 1");
    assert_eq!(binary_version(&binary, PROBE_TIMEOUT_MS).await, None);
}

#[tokio::test]
async fn a_binary_printing_garbage_has_no_version() {
    let dir = tempfile::tempdir().expect("temp dir");
    let binary = fake_binary(dir.path(), "echo 'not/a/version'", "echo not/a/version");
    assert_eq!(binary_version(&binary, PROBE_TIMEOUT_MS).await, None);
}

#[tokio::test]
async fn a_slow_binary_times_out() {
    let dir = tempfile::tempdir().expect("temp dir");
    let binary = fake_binary(
        dir.path(),
        "sleep 3",
        r"C:\Windows\System32\PING.EXE -n 4 127.0.0.1 >nul",
    );
    let started = std::time::Instant::now();
    assert_eq!(binary_version(&binary, SLOW_BINARY_TIMEOUT_MS).await, None);
    assert!(started.elapsed() < Duration::from_secs(3));
}
