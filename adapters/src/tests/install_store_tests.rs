use std::{
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use super::*;
use crate::install::InstallError;

fn mode_of(path: &Path) -> u32 {
    std::fs::metadata(path)
        .expect("the path exists")
        .permissions()
        .mode()
        & 0o777
}

#[tokio::test]
async fn a_backup_copies_every_existing_file_into_a_private_stamp_dir() {
    let dir = tempfile::tempdir().expect("temp dir");
    let binary = dir.path().join("pm3");
    let config = dir.path().join("config.yaml");
    std::fs::write(&binary, "binary").expect("write binary");
    std::fs::write(&config, "config").expect("write config");
    let stamp = back_up(
        &[binary.clone(), config],
        &dir.path().join("backups"),
        "20260730T133344Z",
    )
    .await
    .expect("the backup succeeds");
    assert_eq!(stamp, dir.path().join("backups/20260730T133344Z"));
    assert_eq!(mode_of(&dir.path().join("backups")), 0o700);
    assert_eq!(mode_of(&stamp), 0o700);
    assert_eq!(
        std::fs::read_to_string(stamp.join("pm3")).expect("binary copy"),
        "binary"
    );
    assert_eq!(mode_of(&stamp.join("pm3")), 0o600);
    assert_eq!(mode_of(&stamp.join("config.yaml")), 0o600);
}

#[tokio::test]
async fn a_backup_skips_files_that_do_not_exist() {
    let dir = tempfile::tempdir().expect("temp dir");
    let stamp = back_up(
        &[dir.path().join("missing")],
        &dir.path().join("backups"),
        "stamp",
    )
    .await
    .expect("a missing file is not an error");
    assert!(
        std::fs::read_dir(&stamp)
            .expect("the stamp dir")
            .next()
            .is_none()
    );
}

#[tokio::test]
async fn a_backup_reports_a_root_that_is_not_a_directory() {
    let dir = tempfile::tempdir().expect("temp dir");
    let file = dir.path().join("file");
    std::fs::write(&file, "body").expect("write file");
    let error = back_up(&[], &file, "stamp").await.unwrap_err();
    assert!(
        error
            .to_string()
            .starts_with("cannot prepare the backup directory"),
        "got: {error}"
    );
}

#[tokio::test]
async fn a_backup_reports_a_source_it_cannot_copy() {
    let dir = tempfile::tempdir().expect("temp dir");
    let error = back_up(
        &[dir.path().to_path_buf()],
        &dir.path().join("backups"),
        "stamp",
    )
    .await
    .unwrap_err();
    assert!(
        error.to_string().starts_with("cannot back up '"),
        "got: {error}"
    );
}

#[tokio::test]
async fn a_backup_reports_a_source_without_a_file_name() {
    let dir = tempfile::tempdir().expect("temp dir");
    let error = back_up(&[PathBuf::from("/")], &dir.path().join("backups"), "stamp")
        .await
        .unwrap_err();
    assert!(
        error.to_string().contains("has no file name"),
        "got: {error}"
    );
}

#[tokio::test]
async fn a_backup_reports_an_unreadable_source() {
    let dir = tempfile::tempdir().expect("temp dir");
    let locked = dir.path().join("locked");
    std::fs::create_dir(&locked).expect("mkdir");
    std::fs::write(locked.join("secret"), "body").expect("write");
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).expect("lock");
    let outcome = back_up(
        &[locked.join("secret")],
        &dir.path().join("backups"),
        "stamp",
    )
    .await;
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o700)).expect("unlock");
    let error = outcome.unwrap_err();
    assert!(
        error.to_string().starts_with("cannot back up '"),
        "got: {error}"
    );
}

#[tokio::test]
async fn restricting_a_missing_directory_is_an_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let missing = dir.path().join("missing");
    let error = restrict_dir(&missing).await.unwrap_err();
    assert!(
        error
            .to_string()
            .starts_with("cannot prepare the backup directory"),
        "got: {error}"
    );
}

#[tokio::test]
async fn restricting_a_missing_file_is_an_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let missing = dir.path().join("missing");
    let error = restrict_file(&missing).await.unwrap_err();
    assert!(
        error.to_string().starts_with("cannot back up '"),
        "got: {error}"
    );
}

#[tokio::test]
async fn a_replacement_lands_atomically_next_to_the_destination() {
    let dir = tempfile::tempdir().expect("temp dir");
    let source = dir.path().join("new-pm3");
    std::fs::write(&source, "new binary").expect("write source");
    let destination = dir.path().join("bin/pm3");
    std::fs::create_dir_all(dir.path().join("bin")).expect("mkdir");
    std::fs::write(&destination, "old binary").expect("write destination");
    replace_binary(&source, &destination)
        .await
        .expect("the replacement succeeds");
    assert_eq!(
        std::fs::read_to_string(&destination).expect("destination"),
        "new binary"
    );
    assert!(!dir.path().join("bin/pm3.incoming").exists());
}

#[tokio::test]
async fn a_replacement_creates_a_missing_parent_directory() {
    let dir = tempfile::tempdir().expect("temp dir");
    let source = dir.path().join("new-pm3");
    std::fs::write(&source, "new binary").expect("write source");
    let destination = dir.path().join("bin/pm3");
    replace_binary(&source, &destination)
        .await
        .expect("the parent is created");
    assert_eq!(
        std::fs::read_to_string(&destination).expect("destination"),
        "new binary"
    );
}

#[tokio::test]
async fn a_replacement_reports_a_missing_source() {
    let dir = tempfile::tempdir().expect("temp dir");
    let destination = dir.path().join("pm3");
    let error = replace_binary(&dir.path().join("missing"), &destination)
        .await
        .unwrap_err();
    assert!(
        error.to_string().starts_with("cannot replace '"),
        "got: {error}"
    );
    assert!(!destination.exists());
}

#[tokio::test]
async fn a_replacement_reports_a_parent_that_is_a_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let parent = dir.path().join("file");
    std::fs::write(&parent, "body").expect("write file");
    let source = dir.path().join("new-pm3");
    std::fs::write(&source, "new binary").expect("write source");
    let error = replace_binary(&source, &parent.join("pm3"))
        .await
        .unwrap_err();
    assert!(
        error.to_string().starts_with("cannot replace '"),
        "got: {error}"
    );
}

#[tokio::test]
async fn a_replacement_reports_a_destination_it_cannot_rename_over() {
    let dir = tempfile::tempdir().expect("temp dir");
    let source = dir.path().join("new-pm3");
    std::fs::write(&source, "new binary").expect("write source");
    let destination = dir.path().join("pm3");
    std::fs::create_dir(&destination).expect("a directory occupies the destination");
    let error = replace_binary(&source, &destination).await.unwrap_err();
    assert!(
        error.to_string().starts_with("cannot replace '"),
        "got: {error}"
    );
}

#[test]
fn every_error_variant_renders_its_message() {
    let variants = [
        InstallError::DestinationHome,
        InstallError::BackupDirectory {
            path: "p".to_string(),
            reason: "r".to_string(),
        },
        InstallError::Backup {
            path: "p".to_string(),
            reason: "r".to_string(),
        },
        InstallError::Replace {
            path: "p".to_string(),
            reason: "r".to_string(),
        },
    ];
    let rendered: Vec<String> = variants.iter().map(ToString::to_string).collect();
    assert_eq!(rendered.len(), 4);
    assert!(
        rendered
            .iter()
            .all(|line| line.contains('p') || line.contains("HOME"))
    );
}

#[test]
fn a_staged_path_sits_next_to_the_destination() {
    assert_eq!(
        staged_path(Path::new("/home/dev/bin/pm3")),
        PathBuf::from("/home/dev/bin/pm3.incoming")
    );
}

#[tokio::test]
async fn a_replacement_without_a_parent_directory_skips_the_mkdir() {
    let dir = tempfile::tempdir().expect("temp dir");
    let source = dir.path().join("new-pm3");
    std::fs::write(&source, "new binary").expect("write source");
    let error = replace_binary(&source, Path::new("/")).await.unwrap_err();
    assert!(
        error.to_string().starts_with("cannot replace '"),
        "got: {error}"
    );
}

#[tokio::test]
async fn a_backup_reports_a_stamp_path_that_is_a_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path().join("backups");
    std::fs::create_dir_all(&root).expect("prepare the root");
    std::fs::write(root.join("stamp"), "occupied").expect("a file occupies the stamp path");
    let error = back_up(&[], &root, "stamp").await.unwrap_err();
    assert!(
        error
            .to_string()
            .starts_with("cannot prepare the backup directory"),
        "got: {error}"
    );
}
