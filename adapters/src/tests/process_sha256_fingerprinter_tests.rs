use std::fs;

use super::*;

const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
const ABC_SHA256: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

fn file_with(contents: &[u8]) -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("program");
    fs::write(&path, contents).expect("should write the fixture");
    let path = path.to_string_lossy().into_owned();
    (dir, path)
}

#[test]
fn the_digest_of_the_empty_text_matches_the_published_sha256_vector() {
    assert_eq!(Sha256Fingerprinter.digest(""), EMPTY_SHA256);
}

#[test]
fn the_digest_matches_the_published_sha256_vector() {
    assert_eq!(Sha256Fingerprinter.digest("abc"), ABC_SHA256);
}

#[test]
fn the_same_text_always_digests_the_same_way() {
    let first = Sha256Fingerprinter.digest("api --port=8080");
    let second = Sha256Fingerprinter.digest("api --port=8080");
    assert_eq!(first, second);
}

#[test]
fn different_texts_digest_differently() {
    let first = Sha256Fingerprinter.digest("api --port=8080");
    let second = Sha256Fingerprinter.digest("api --port=8081");
    assert_ne!(first, second);
}

#[tokio::test]
async fn a_file_digests_by_its_contents() {
    let (_dir, path) = file_with(b"abc");
    let digest = Sha256Fingerprinter
        .file_digest(&path)
        .await
        .expect("the fixture is readable");
    assert_eq!(digest, ABC_SHA256);
}

#[tokio::test]
async fn an_empty_file_digests_to_the_empty_vector() {
    let (_dir, path) = file_with(b"");
    let digest = Sha256Fingerprinter
        .file_digest(&path)
        .await
        .expect("the fixture is readable");
    assert_eq!(digest, EMPTY_SHA256);
}

#[tokio::test]
async fn rewriting_a_file_changes_its_digest() {
    let (dir, path) = file_with(b"abc");
    let before = Sha256Fingerprinter
        .file_digest(&path)
        .await
        .expect("readable");
    fs::write(dir.path().join("program"), b"abd").expect("should rewrite the fixture");
    let after = Sha256Fingerprinter
        .file_digest(&path)
        .await
        .expect("readable");
    assert_ne!(before, after);
}

#[tokio::test]
async fn a_missing_file_reports_its_path() {
    let err = Sha256Fingerprinter
        .file_digest("/nonexistent/pm3-program")
        .await
        .unwrap_err()
        .to_string();
    assert!(
        err.starts_with("cannot digest '/nonexistent/pm3-program'"),
        "got: {err}"
    );
}
