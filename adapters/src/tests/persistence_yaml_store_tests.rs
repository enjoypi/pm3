use tempfile::TempDir;
use usecases::DumpStore;

use super::*;
use crate::{
    process_records::{sample_record, sample_runtime, stopped_record},
    spec_sources::{register_service, spec_source_in, write_service_file},
};

struct Fixture {
    dir: TempDir,
    store: YamlDumpStore,
    source: SpecSource,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().expect("create temp dir");
    let source = spec_source_in(dir.path());
    let store = YamlDumpStore::new(dir.path().join("dump.yaml"), source.clone());
    Fixture { dir, store, source }
}

fn store_at(root: &TempDir, path: PathBuf) -> YamlDumpStore {
    YamlDumpStore::new(path, spec_source_in(root.path()))
}

async fn rejoined(fixture: &Fixture, name: &str) -> ProcessRecord {
    let mut spec = fixture
        .source
        .resolve_service(name)
        .await
        .expect("the service file should resolve");
    materialise_workspace(&mut spec).await;
    ProcessRecord {
        spec,
        runtime: sample_runtime(name),
    }
}

async fn saved_yaml(store: &YamlDumpStore, records: &[ProcessRecord]) -> String {
    store.save(records).await.expect("should save");
    tokio::fs::read_to_string(store.path())
        .await
        .expect("should read back")
}

#[tokio::test]
async fn load_reports_no_records_when_the_dump_is_absent() {
    let fixture = fixture();
    let records = fixture.store.load().await.expect("should load");
    assert!(records.is_empty(), "got: {records:?}");
}

#[tokio::test]
async fn save_creates_the_dump_file() {
    let fixture = fixture();
    fixture
        .store
        .save(&[sample_record("web")])
        .await
        .expect("save");
    assert!(fixture.store.path().is_file(), "dump file should exist");
}

#[tokio::test]
async fn save_leaves_no_temporary_file_behind() {
    let fixture = fixture();
    fixture
        .store
        .save(&[sample_record("web")])
        .await
        .expect("save");
    assert!(
        !fixture.dir.path().join("dump.yaml.tmp").exists(),
        "temp file should be renamed away"
    );
}

#[tokio::test]
async fn save_keeps_the_launch_parameters_out_of_the_dump() {
    let fixture = fixture();
    let yaml = saved_yaml(&fixture.store, &[sample_record("web")]).await;
    assert!(!yaml.contains("script"), "got: {yaml}");
    assert!(!yaml.contains("sandbox"), "got: {yaml}");
}

#[tokio::test]
async fn save_then_load_rejoins_a_running_app_with_its_service_file() {
    let fixture = fixture();
    register_service(&fixture.source, "web");
    fixture
        .store
        .save(&[sample_record("web")])
        .await
        .expect("save");
    let records = fixture.store.load().await.expect("load");
    assert_eq!(records, vec![rejoined(&fixture, "web").await]);
}

#[tokio::test]
async fn save_then_load_round_trips_an_idle_app() {
    let fixture = fixture();
    register_service(&fixture.source, "web");
    fixture
        .store
        .save(&[stopped_record("web")])
        .await
        .expect("save");
    let records = fixture.store.load().await.expect("load");
    assert_eq!(records[0].runtime, stopped_record("web").runtime);
}

#[tokio::test]
async fn load_expands_the_service_cwd_placeholder() {
    let fixture = fixture();
    write_service_file(
        &fixture.source,
        "web",
        "name: \"web\"\nscript: \"/bin/sh\"\nargs:\n  - \"${PM3_SERVICE_CWD}\"\n",
    );
    fixture
        .store
        .save(&[sample_record("web")])
        .await
        .expect("save");
    let records = fixture.store.load().await.expect("load");
    assert_eq!(records[0].spec.args, vec![records[0].spec.cwd.clone()]);
}

#[tokio::test]
async fn load_prepares_the_working_directory() {
    let fixture = fixture();
    register_service(&fixture.source, "web");
    fixture
        .store
        .save(&[sample_record("web")])
        .await
        .expect("save");
    fixture.store.load().await.expect("load");
    assert!(
        fixture.dir.path().join("web").is_dir(),
        "the working directory should exist"
    );
}

#[tokio::test]
async fn save_then_load_keeps_the_declared_order() {
    let fixture = fixture();
    register_service(&fixture.source, "web");
    register_service(&fixture.source, "db");
    fixture
        .store
        .save(&[sample_record("web"), sample_record("db")])
        .await
        .expect("save");
    let loaded = fixture.store.load().await.expect("load");
    let names: Vec<&str> = loaded
        .iter()
        .map(|record| record.runtime.name.as_str())
        .collect();
    assert_eq!(names, vec!["web", "db"]);
}

#[tokio::test]
async fn save_replaces_a_previous_dump() {
    let fixture = fixture();
    register_service(&fixture.source, "db");
    fixture
        .store
        .save(&[sample_record("web")])
        .await
        .expect("first save");
    fixture
        .store
        .save(&[sample_record("db")])
        .await
        .expect("second save");
    let records = fixture.store.load().await.expect("load");
    assert_eq!(records, vec![rejoined(&fixture, "db").await]);
}

#[tokio::test]
async fn save_writes_an_empty_document_for_no_records() {
    let fixture = fixture();
    let yaml = saved_yaml(&fixture.store, &[]).await;
    assert!(yaml.contains("services"), "got: {yaml}");
    assert!(fixture.store.load().await.expect("load").is_empty());
}

#[tokio::test]
async fn load_skips_an_app_without_a_service_file() {
    let fixture = fixture();
    fixture
        .store
        .save(&[sample_record("web")])
        .await
        .expect("save");
    assert!(fixture.store.load().await.expect("load").is_empty());
}

#[tokio::test]
async fn load_skips_an_app_whose_service_file_is_broken() {
    let fixture = fixture();
    write_service_file(&fixture.source, "web", "{{not yaml");
    fixture
        .store
        .save(&[sample_record("web")])
        .await
        .expect("save");
    assert!(fixture.store.load().await.expect("load").is_empty());
}

#[tokio::test]
async fn load_reports_a_broken_document() {
    let fixture = fixture();
    tokio::fs::write(fixture.store.path(), "{{not yaml")
        .await
        .expect("write");
    let err = fixture.store.load().await.unwrap_err().to_string();
    assert!(err.contains("cannot read state file"), "got: {err}");
}

#[tokio::test]
async fn load_skips_a_record_with_an_unknown_status() {
    let fixture = fixture();
    register_service(&fixture.source, "web");
    let yaml = saved_yaml(&fixture.store, &[sample_record("web")]).await;
    tokio::fs::write(fixture.store.path(), yaml.replace("online", "zombie"))
        .await
        .expect("write");
    assert!(fixture.store.load().await.expect("load").is_empty());
}

#[tokio::test]
async fn load_skips_a_running_record_without_a_pid() {
    let fixture = fixture();
    register_service(&fixture.source, "web");
    let yaml = saved_yaml(&fixture.store, &[sample_record("web")]).await;
    let mut doc: DumpDocument = serde_yaml2::from_str(&yaml).expect("the saved dump parses");
    doc.services[0].runtime.pid = None;
    let broken = serde_yaml2::to_string(&doc).expect("the edited dump serializes");
    tokio::fs::write(fixture.store.path(), broken)
        .await
        .expect("write");
    assert!(fixture.store.load().await.expect("load").is_empty());
}

#[tokio::test]
async fn load_keeps_the_records_around_a_corrupt_one() {
    let fixture = fixture();
    register_service(&fixture.source, "web");
    register_service(&fixture.source, "db");
    let yaml = saved_yaml(&fixture.store, &[sample_record("web"), sample_record("db")]).await;
    tokio::fs::write(fixture.store.path(), yaml.replacen("online", "zombie", 1))
        .await
        .expect("write");
    let loaded = fixture.store.load().await.expect("load");
    let names: Vec<&str> = loaded
        .iter()
        .map(|record| record.runtime.name.as_str())
        .collect();
    assert_eq!(names, vec!["db"]);
}

#[tokio::test]
async fn load_reports_a_dump_path_that_is_a_directory() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("dump.yaml");
    std::fs::create_dir(&path).expect("create dir in place of the dump");
    let err = store_at(&dir, path).load().await.unwrap_err().to_string();
    assert!(err.contains("cannot read state file"), "got: {err}");
}

#[tokio::test]
async fn save_reports_a_missing_parent_directory() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = store_at(&dir, dir.path().join("absent").join("dump.yaml"));
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
    let err = store_at(&dir, path)
        .save(&[sample_record("web")])
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot write state file"), "got: {err}");
}
