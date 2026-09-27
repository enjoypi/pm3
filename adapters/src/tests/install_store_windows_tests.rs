#![cfg(windows)]
use std::os::windows::fs::OpenOptionsExt as _;

use super::*;

const NO_SHARING: u32 = 0;

struct Install {
    dir: tempfile::TempDir,
    source: PathBuf,
}

fn install() -> Install {
    let dir = tempfile::tempdir().expect("temp dir");
    let source = dir.path().join("new-pm3");
    std::fs::write(&source, "new binary").expect("write source");
    Install { dir, source }
}

impl Install {
    fn destination(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    async fn replace(&self, destination: &Path) -> String {
        replace_binary(&self.source, destination)
            .await
            .unwrap_err()
            .to_string()
    }
}

#[tokio::test]
async fn a_replacement_moves_the_running_binary_aside() {
    let install = install();
    let destination = install.destination("pm3.exe");
    std::fs::write(&destination, "old binary").expect("write destination");
    replace_binary(&install.source, &destination)
        .await
        .expect("the replacement succeeds");
    assert_eq!(
        std::fs::read_to_string(&destination).expect("destination"),
        "new binary"
    );
    assert_eq!(
        std::fs::read_to_string(retired_path(&destination)).expect("retired copy"),
        "old binary"
    );
}

#[tokio::test]
async fn a_replacement_clears_the_copy_an_earlier_one_retired() {
    let install = install();
    let destination = install.destination("pm3.exe");
    std::fs::write(&destination, "old binary").expect("write destination");
    std::fs::write(retired_path(&destination), "older binary").expect("write retired");
    replace_binary(&install.source, &destination)
        .await
        .expect("the replacement succeeds");
    assert_eq!(
        std::fs::read_to_string(retired_path(&destination)).expect("retired copy"),
        "old binary"
    );
}

#[tokio::test]
async fn a_retired_slot_pm3_cannot_clear_stops_the_replacement() {
    let install = install();
    let destination = install.destination("pm3.exe");
    std::fs::create_dir(retired_path(&destination)).expect("a directory occupies the slot");
    let error = install.replace(&destination).await;
    assert!(error.contains(".retired"), "got: {error}");
}

#[tokio::test]
async fn a_binary_held_open_without_sharing_is_not_moved_aside() {
    let install = install();
    let destination = install.destination("pm3.exe");
    std::fs::write(&destination, "old binary").expect("write destination");
    let held = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(NO_SHARING)
        .open(&destination)
        .expect("hold the destination");
    let error = install.replace(&destination).await;
    drop(held);
    assert!(error.starts_with("cannot replace '"), "got: {error}");
    assert_eq!(
        std::fs::read_to_string(&destination).expect("destination"),
        "old binary"
    );
}

#[tokio::test]
async fn a_destination_whose_presence_cannot_be_read_stops_the_replacement() {
    let install = install();
    let error = install.replace(&install.destination("pm3:")).await;
    assert!(error.contains("(os error 123)"), "got: {error}");
}

#[tokio::test]
async fn a_destination_the_staged_copy_cannot_be_renamed_onto_stops_the_replacement() {
    let install = install();
    let error = install.replace(&install.destination("pm3:stream")).await;
    assert!(error.contains("(os error 87)"), "got: {error}");
}

#[tokio::test]
async fn a_destination_occupied_by_a_directory_is_moved_aside() {
    let install = install();
    let destination = install.destination("pm3.exe");
    std::fs::create_dir(&destination).expect("a directory occupies the destination");
    replace_binary(&install.source, &destination)
        .await
        .expect("the replacement succeeds");
    assert!(retired_path(&destination).is_dir());
}

#[test]
fn a_retired_path_sits_next_to_the_destination() {
    assert_eq!(
        retired_path(Path::new(r"C:\pm3\pm3.exe")),
        PathBuf::from(r"C:\pm3\pm3.exe.retired")
    );
}

#[tokio::test]
async fn restricting_a_directory_is_left_to_ntfs() {
    let dir = tempfile::tempdir().expect("temp dir");
    restrict_dir(&dir.path().join("missing"))
        .await
        .expect("NTFS needs no restriction");
    restrict_file(&dir.path().join("missing"))
        .await
        .expect("NTFS needs no restriction");
}
