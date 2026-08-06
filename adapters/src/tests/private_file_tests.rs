use std::os::unix::fs::PermissionsExt as _;

use super::*;

fn mode_of(path: &Path) -> u32 {
    std::fs::metadata(path)
        .expect("the file should exist")
        .permissions()
        .mode()
        & 0o777
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

#[cfg(target_os = "linux")]
#[tokio::test]
async fn a_device_that_cannot_take_the_bytes_surfaces_as_an_error() {
    let err = write_private(Path::new("/dev/full"), "spill")
        .await
        .unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::StorageFull);
}
