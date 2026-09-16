#![cfg(unix)]

use super::*;

fn mode_of(path: &Path) -> u32 {
    full_mode_of(path) & 0o777
}

fn full_mode_of(path: &Path) -> u32 {
    std::fs::metadata(path)
        .expect("the file should exist")
        .permissions()
        .mode()
        & 0o7777
}

#[tokio::test]
async fn a_written_file_is_readable_by_its_owner_alone() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("dump.yaml");
    write_private(&path, "services: []\n")
        .await
        .expect("should write");
    assert_eq!(mode_of(&path), OWNER_ONLY_FILE);
    assert_eq!(
        std::fs::read_to_string(&path).expect("should read"),
        "services: []\n"
    );
}

#[tokio::test]
async fn writing_twice_replaces_the_previous_contents() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("dump.yaml");
    write_private(&path, "first\n").await.expect("should write");
    write_private(&path, "second\n")
        .await
        .expect("should rewrite");
    assert_eq!(
        std::fs::read_to_string(&path).expect("should read"),
        "second\n"
    );
}

#[tokio::test]
async fn a_missing_parent_directory_surfaces_as_an_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("absent").join("dump.yaml");
    let err = write_private(&path, "x").await.unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
}

#[tokio::test]
async fn an_appended_log_is_readable_by_its_owner_alone() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("app-out.log");
    let file = append_private(&path).await.expect("should open");
    drop(file);
    assert_eq!(mode_of(&path), OWNER_ONLY_FILE);
}

#[tokio::test]
async fn an_appended_log_refuses_a_missing_directory() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("absent").join("app-out.log");
    let err = append_private(&path).await.unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
}

#[test]
fn a_blocking_append_keeps_the_same_permissions() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("pm3.log");
    let file = append_private_blocking(&path).expect("should open");
    drop(file);
    assert_eq!(mode_of(&path), OWNER_ONLY_FILE);
}

#[test]
fn a_blocking_append_refuses_a_missing_directory() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("absent").join("pm3.log");
    let err = append_private_blocking(&path).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
}

#[tokio::test]
async fn a_handle_that_refuses_the_bytes_surfaces_as_an_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("dump.yaml");
    std::fs::write(&path, "seed").expect("seed the file");
    let readable = std::fs::File::open(&path).expect("open the file for reading only");
    assert!(
        fill(File::from_std(readable), b"spill").await.is_err(),
        "a refused write must reach the caller instead of passing for a saved file"
    );
}

#[tokio::test]
async fn a_sweep_proof_file_is_owner_only_and_carries_the_sticky_bit() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("pm3.pid");
    write_sweep_proof(&path, "4242")
        .await
        .expect("should write");
    let mode = full_mode_of(&path);
    assert_eq!(
        mode, SWEEP_PROOF_FILE,
        "systemd-tmpfiles skips a sticky runtime file, got: {mode:o}"
    );
    assert_eq!(std::fs::read_to_string(&path).expect("should read"), "4242");
}

#[tokio::test]
async fn a_sweep_proof_file_refuses_a_missing_directory() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("absent").join("pm3.pid");
    let err = write_sweep_proof(&path, "4242").await.unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
}
