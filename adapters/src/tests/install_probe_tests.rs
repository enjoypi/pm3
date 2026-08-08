use std::{
    os::unix::fs::PermissionsExt as _,
    path::{Path, PathBuf},
    time::Duration,
};

use super::*;

fn fake_binary(dir: &Path, body: &str) -> PathBuf {
    let path = dir.join("old-pm3");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write the fake binary");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    path
}

#[tokio::test]
async fn a_binary_that_prints_a_version_is_named_by_it() {
    let dir = tempfile::tempdir().expect("temp dir");
    let binary = fake_binary(dir.path(), "echo 'pm3 1.8.0'");
    assert_eq!(binary_version(&binary).await, Some("1.8.0".to_string()));
}

#[tokio::test]
async fn a_missing_binary_has_no_version() {
    let dir = tempfile::tempdir().expect("temp dir");
    assert_eq!(binary_version(&dir.path().join("missing")).await, None);
}

#[tokio::test]
async fn a_failing_binary_has_no_version() {
    let dir = tempfile::tempdir().expect("temp dir");
    let binary = fake_binary(dir.path(), "exit 1");
    assert_eq!(binary_version(&binary).await, None);
}

#[tokio::test]
async fn a_binary_printing_garbage_has_no_version() {
    let dir = tempfile::tempdir().expect("temp dir");
    let binary = fake_binary(dir.path(), "echo 'not/a/version'");
    assert_eq!(binary_version(&binary).await, None);
}

#[tokio::test]
async fn a_slow_binary_times_out() {
    let dir = tempfile::tempdir().expect("temp dir");
    let binary = fake_binary(dir.path(), "sleep 3");
    let started = std::time::Instant::now();
    assert_eq!(binary_version(&binary).await, None);
    assert!(started.elapsed() < Duration::from_secs(3));
}
