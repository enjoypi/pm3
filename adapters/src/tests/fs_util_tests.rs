use super::*;

#[test]
fn a_suffix_is_appended_to_the_whole_file_name() {
    assert_eq!(
        with_suffix(Path::new("/srv/dump.yaml"), ".tmp"),
        PathBuf::from("/srv/dump.yaml.tmp")
    );
}

#[tokio::test]
async fn a_missing_file_reads_as_nothing() {
    let dir = tempfile::tempdir().expect("temp dir");
    let read = read_optional(&dir.path().join("absent")).await;
    assert_eq!(read.expect("a missing file is not an error"), None);
}

#[tokio::test]
async fn a_present_file_reads_as_its_text() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("present");
    std::fs::write(&path, "text").expect("write the file");
    let read = read_optional(&path).await;
    assert_eq!(read.expect("read the file"), Some("text".to_string()));
}

#[tokio::test]
async fn a_directory_cannot_be_read_as_text() {
    let dir = tempfile::tempdir().expect("temp dir");
    assert!(read_optional(dir.path()).await.is_err());
}

#[tokio::test]
async fn removing_a_missing_file_succeeds() {
    let dir = tempfile::tempdir().expect("temp dir");
    remove_if_present(&dir.path().join("absent"))
        .await
        .expect("a missing file is already removed");
}

#[tokio::test]
async fn removing_a_present_file_deletes_it() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("present");
    std::fs::write(&path, "text").expect("write the file");
    remove_if_present(&path).await.expect("remove the file");
    assert!(!path.exists());
}

#[tokio::test]
async fn removing_a_directory_reports_the_failure() {
    let dir = tempfile::tempdir().expect("temp dir");
    assert!(remove_if_present(dir.path()).await.is_err());
}
