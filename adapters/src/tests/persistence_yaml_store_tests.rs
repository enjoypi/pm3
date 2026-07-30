use std::path::PathBuf;

use tempfile::TempDir;
use usecases::DumpStore;

use super::*;
use crate::process_records::{sample_record, stopped_record};

fn store_in_temp_dir() -> (TempDir, YamlDumpStore) {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = YamlDumpStore::new(dir.path().join("dump.yaml"));
    (dir, store)
}

fn store_at(path: PathBuf) -> YamlDumpStore {
    YamlDumpStore::new(path)
}

async fn saved_yaml(store: &YamlDumpStore, records: &[ProcessRecord]) -> String {
    store.save(records).await.expect("should save");
    tokio::fs::read_to_string(store.path())
        .await
        .expect("should read back")
}

#[tokio::test]
async fn load_reports_no_records_when_the_dump_is_absent() {
    let (_dir, store) = store_in_temp_dir();
    let records = store.load().await.expect("should load");
    assert!(records.is_empty(), "got: {records:?}");
}

#[tokio::test]
async fn save_creates_the_dump_file() {
    let (_dir, store) = store_in_temp_dir();
    store.save(&[sample_record("web")]).await.expect("save");
    assert!(store.path().is_file(), "dump file should exist");
}

#[tokio::test]
async fn save_leaves_no_temporary_file_behind() {
    let (dir, store) = store_in_temp_dir();
    store.save(&[sample_record("web")]).await.expect("save");
    assert!(
        !dir.path().join("dump.yaml.tmp").exists(),
        "temp file should be renamed away"
    );
}

#[tokio::test]
async fn save_then_load_round_trips_a_running_app() {
    let (_dir, store) = store_in_temp_dir();
    store.save(&[sample_record("web")]).await.expect("save");
    let records = store.load().await.expect("load");
    assert_eq!(records, vec![sample_record("web")]);
}

#[tokio::test]
async fn save_then_load_round_trips_an_idle_app() {
    let (_dir, store) = store_in_temp_dir();
    store.save(&[stopped_record("web")]).await.expect("save");
    let records = store.load().await.expect("load");
    assert_eq!(records, vec![stopped_record("web")]);
}

#[tokio::test]
async fn save_then_load_round_trips_an_app_without_collections() {
    let (_dir, store) = store_in_temp_dir();
    let mut bare = sample_record("web");
    bare.spec.args = Vec::new();
    bare.spec.env = Vec::new();
    bare.spec.depends_on = Vec::new();
    bare.spec.sandbox.writable_roots = Vec::new();
    store.save(&[bare.clone()]).await.expect("save");
    assert_eq!(store.load().await.expect("load"), vec![bare]);
}

#[tokio::test]
async fn save_then_load_keeps_the_declared_order() {
    let (_dir, store) = store_in_temp_dir();
    let records = vec![sample_record("web"), sample_record("db")];
    store.save(&records).await.expect("save");
    let loaded = store.load().await.expect("load");
    assert_eq!(loaded, records);
}

#[tokio::test]
async fn save_replaces_a_previous_dump() {
    let (_dir, store) = store_in_temp_dir();
    store
        .save(&[sample_record("web")])
        .await
        .expect("first save");
    store
        .save(&[sample_record("db")])
        .await
        .expect("second save");
    let records = store.load().await.expect("load");
    assert_eq!(records, vec![sample_record("db")]);
}

#[tokio::test]
async fn save_writes_an_empty_document_for_no_records() {
    let (_dir, store) = store_in_temp_dir();
    let yaml = saved_yaml(&store, &[]).await;
    assert!(yaml.contains("apps"), "got: {yaml}");
    assert!(store.load().await.expect("load").is_empty());
}

#[tokio::test]
async fn load_reports_a_broken_document() {
    let (dir, store) = store_in_temp_dir();
    tokio::fs::write(dir.path().join("dump.yaml"), "{{not yaml")
        .await
        .expect("write");
    let err = store.load().await.unwrap_err().to_string();
    assert!(err.contains("cannot read state file"), "got: {err}");
}

#[tokio::test]
async fn load_reports_an_unknown_status() {
    let (_dir, store) = store_in_temp_dir();
    let yaml = saved_yaml(&store, &[sample_record("web")]).await;
    tokio::fs::write(store.path(), yaml.replace("online", "zombie"))
        .await
        .expect("write");
    let err = store.load().await.unwrap_err().to_string();
    assert!(err.contains("unknown status 'zombie'"), "got: {err}");
}

#[tokio::test]
async fn load_reports_an_unknown_sandbox_mode() {
    let (_dir, store) = store_in_temp_dir();
    let yaml = saved_yaml(&store, &[sample_record("web")]).await;
    tokio::fs::write(store.path(), yaml.replace("workspace-write", "yolo"))
        .await
        .expect("write");
    let err = store.load().await.unwrap_err().to_string();
    assert!(err.contains("unknown sandbox mode 'yolo'"), "got: {err}");
}

#[tokio::test]
async fn load_reports_a_dump_path_that_is_a_directory() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("dump.yaml");
    std::fs::create_dir(&path).expect("create dir in place of the dump");
    let err = store_at(path).load().await.unwrap_err().to_string();
    assert!(err.contains("cannot read state file"), "got: {err}");
}

#[tokio::test]
async fn save_reports_a_missing_parent_directory() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = store_at(dir.path().join("absent").join("dump.yaml"));
    let err = store
        .save(&[sample_record("web")])
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot write state file"), "got: {err}");
}

#[tokio::test]
async fn save_reports_a_dump_path_blocked_by_a_directory() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("dump.yaml");
    std::fs::create_dir(&path).expect("create dir in place of the dump");
    std::fs::write(path.join("occupant"), "blocked").expect("fill the directory");
    let err = store_at(path)
        .save(&[sample_record("web")])
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot write state file"), "got: {err}");
}
