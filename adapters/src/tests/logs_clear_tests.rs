use super::*;

#[tokio::test]
async fn clearing_an_existing_log_truncates_it_in_place() {
    let dir = tempfile::tempdir().expect("temp dir");
    let log = dir.path().join("web-out.log");
    std::fs::write(&log, b"first\nsecond\n").expect("seed the log");
    clear_log(&log).await.expect("should clear");
    assert_eq!(std::fs::metadata(&log).expect("stat").len(), 0);
}

#[tokio::test]
async fn clearing_a_missing_log_names_the_path() {
    let dir = tempfile::tempdir().expect("temp dir");
    let log = dir.path().join("ghost-out.log");
    let err = clear_log(&log).await.unwrap_err();
    assert_eq!(
        err.to_string(),
        format!(
            "cannot clear log file '{}': No such file or directory (os error 2)",
            log.to_string_lossy()
        )
    );
}

#[tokio::test]
async fn clearing_a_directory_is_an_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let err = clear_log(dir.path()).await.unwrap_err();
    assert!(
        err.to_string().contains("cannot clear log file"),
        "got: {err}"
    );
}
