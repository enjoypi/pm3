use tempfile::TempDir;
use usecases::{DumpContents, DumpStore, StrandedProcess};

use super::*;
use crate::{
    process_records::{
        SAMPLE_BOOT, SAMPLE_PID, SAMPLE_TOKEN, sample_record, sample_runtime, stopped_record,
    },
    spec_sources::{register_service, spec_source_in, write_env_file, write_service_file},
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
    let spec = fixture
        .source
        .resolve_service(name)
        .await
        .expect("the service file should resolve");
    ProcessRecord {
        spec,
        runtime: sample_runtime(name),
    }
}

async fn saved_yaml(store: &YamlDumpStore, records: &[ProcessRecord]) -> String {
    store.save(records, None).await.expect("should save");
    tokio::fs::read_to_string(store.path())
        .await
        .expect("should read back")
}

#[tokio::test]
async fn load_reports_no_records_when_the_dump_is_absent() {
    let fixture = fixture();
    let loaded = fixture.store.load().await.expect("should load");
    assert_eq!(loaded, DumpContents::default(), "got: {loaded:?}");
}

#[tokio::test]
async fn save_creates_the_dump_file() {
    let fixture = fixture();
    fixture
        .store
        .save(&[sample_record("web")], None)
        .await
        .expect("save");
    assert!(fixture.store.path().is_file(), "dump file should exist");
}

#[tokio::test]
async fn save_leaves_no_temporary_file_behind() {
    let fixture = fixture();
    fixture
        .store
        .save(&[sample_record("web")], None)
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
        .save(&[sample_record("web")], None)
        .await
        .expect("save");
    let records = fixture.store.load().await.expect("load").records;
    assert_eq!(records, vec![rejoined(&fixture, "web").await]);
}

#[tokio::test]
async fn save_then_load_round_trips_an_idle_app() {
    let fixture = fixture();
    register_service(&fixture.source, "web");
    fixture
        .store
        .save(&[stopped_record("web")], None)
        .await
        .expect("save");
    let records = fixture.store.load().await.expect("load").records;
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
        .save(&[sample_record("web")], None)
        .await
        .expect("save");
    let records = fixture.store.load().await.expect("load").records;
    assert_eq!(records[0].spec.args, vec![records[0].spec.cwd.clone()]);
}

#[tokio::test]
async fn load_prepares_the_working_directory() {
    let fixture = fixture();
    register_service(&fixture.source, "web");
    fixture
        .store
        .save(&[sample_record("web")], None)
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
        .save(&[sample_record("web"), sample_record("db")], None)
        .await
        .expect("save");
    let loaded = fixture.store.load().await.expect("load");
    let names: Vec<&str> = loaded
        .records
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
        .save(&[sample_record("web")], None)
        .await
        .expect("first save");
    fixture
        .store
        .save(&[sample_record("db")], None)
        .await
        .expect("second save");
    let records = fixture.store.load().await.expect("load").records;
    assert_eq!(records, vec![rejoined(&fixture, "db").await]);
}

#[tokio::test]
async fn save_writes_an_empty_document_for_no_records() {
    let fixture = fixture();
    let yaml = saved_yaml(&fixture.store, &[]).await;
    assert!(yaml.contains("services"), "got: {yaml}");
    assert_eq!(
        fixture.store.load().await.expect("load"),
        DumpContents::default()
    );
}

#[tokio::test]
async fn load_strands_an_app_without_a_service_file() {
    let fixture = fixture();
    fixture
        .store
        .save(&[sample_record("web")], None)
        .await
        .expect("save");
    let loaded = fixture.store.load().await.expect("load");
    assert!(loaded.records.is_empty(), "got: {loaded:?}");
    assert_eq!(
        loaded.stranded,
        vec![StrandedProcess {
            name: "web".to_string(),
            pid: Some(SAMPLE_PID),
            token: Some(SAMPLE_TOKEN.to_string()),
        }],
        "the survivor must be named, or nobody can stop it"
    );
}

#[tokio::test]
async fn load_strands_an_app_whose_service_file_is_broken() {
    let fixture = fixture();
    write_service_file(&fixture.source, "web", "{{not yaml");
    fixture
        .store
        .save(&[sample_record("web")], None)
        .await
        .expect("save");
    let loaded = fixture.store.load().await.expect("load");
    assert!(loaded.records.is_empty(), "got: {loaded:?}");
    assert_eq!(loaded.stranded.len(), 1);
}

#[tokio::test]
async fn load_strands_an_app_whose_environment_file_is_broken() {
    let fixture = fixture();
    register_service(&fixture.source, "web");
    write_env_file(&fixture.source, "web", "TUNNEL_TOKEN\n");
    fixture
        .store
        .save(&[sample_record("web")], None)
        .await
        .expect("save");
    let loaded = fixture.store.load().await.expect("load");
    assert!(loaded.records.is_empty(), "got: {loaded:?}");
    assert_eq!(loaded.stranded[0].pid, Some(SAMPLE_PID));
}

#[tokio::test]
async fn load_strands_an_idle_app_with_no_pid_to_sweep() {
    let fixture = fixture();
    fixture
        .store
        .save(&[stopped_record("web")], None)
        .await
        .expect("save");
    let loaded = fixture.store.load().await.expect("load");
    assert_eq!(
        loaded.stranded,
        vec![StrandedProcess {
            name: "web".to_string(),
            pid: None,
            token: None,
        }]
    );
}

#[cfg(unix)]
#[tokio::test]
async fn load_strands_an_app_whose_writable_root_links_into_a_hidden_root() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let canonical = dir.path().canonicalize().expect("canonical temp dir");
    let source = spec_source_in(&canonical);
    let store = YamlDumpStore::new(canonical.join("dump.yaml"), source.clone());
    let link = canonical.join("data");
    std::os::unix::fs::symlink(&canonical, &link).expect("link into the pm3 home");
    write_service_file(
        &source,
        "web",
        &format!(
            "name: \"web\"\nscript: \"/bin/sh\"\nsandbox:\n  writable_roots:\n    - \"{}\"\n",
            link.display()
        ),
    );
    store
        .save(&[sample_record("web")], None)
        .await
        .expect("save");
    let loaded = store.load().await.expect("load");
    assert!(loaded.records.is_empty(), "got: {loaded:?}");
    assert_eq!(
        loaded.stranded,
        vec![StrandedProcess {
            name: "web".to_string(),
            pid: Some(SAMPLE_PID),
            token: Some(SAMPLE_TOKEN.to_string()),
        }]
    );
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
async fn load_strands_a_record_with_an_unknown_status() {
    let fixture = fixture();
    register_service(&fixture.source, "web");
    let yaml = saved_yaml(&fixture.store, &[sample_record("web")]).await;
    tokio::fs::write(fixture.store.path(), yaml.replace("online", "zombie"))
        .await
        .expect("write");
    let loaded = fixture.store.load().await.expect("load");
    assert!(loaded.records.is_empty(), "got: {loaded:?}");
    assert_eq!(
        loaded.stranded,
        vec![StrandedProcess {
            name: "web".to_string(),
            pid: Some(SAMPLE_PID),
            token: Some(SAMPLE_TOKEN.to_string()),
        }],
        "an undecodable record must still be swept, not dropped"
    );
}

#[tokio::test]
async fn load_strands_a_running_record_without_a_pid() {
    let fixture = fixture();
    register_service(&fixture.source, "web");
    let yaml = saved_yaml(&fixture.store, &[sample_record("web")]).await;
    let mut doc: DumpDocument = serde_yaml2::from_str(&yaml).expect("the saved dump parses");
    doc.services[0].runtime.pid = None;
    let broken = serde_yaml2::to_string(&doc).expect("the edited dump serializes");
    tokio::fs::write(fixture.store.path(), broken)
        .await
        .expect("write");
    let loaded = fixture.store.load().await.expect("load");
    assert!(loaded.records.is_empty(), "got: {loaded:?}");
    assert_eq!(
        loaded.stranded,
        vec![StrandedProcess {
            name: "web".to_string(),
            pid: None,
            token: Some(SAMPLE_TOKEN.to_string()),
        }]
    );
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
        .records
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
        .save(&[sample_record("web")], None)
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
        .save(&[sample_record("web")], None)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot write state file"), "got: {err}");
}

#[tokio::test]
async fn a_saved_dump_carries_the_boot_it_was_written_under() {
    let fixture = fixture();
    register_service(&fixture.source, "web");
    fixture
        .store
        .save(&[sample_record("web")], Some(SAMPLE_BOOT))
        .await
        .expect("save");
    let loaded = fixture.store.load().await.expect("should load");
    assert_eq!(loaded.boot.as_deref(), Some(SAMPLE_BOOT));
}

#[tokio::test]
async fn a_dump_written_before_pm3_recorded_boots_still_loads() {
    let fixture = fixture();
    register_service(&fixture.source, "web");
    let without_boot = saved_yaml(&fixture.store, &[sample_record("web")]).await;
    tokio::fs::write(fixture.store.path(), &without_boot)
        .await
        .expect("seed a dump from an older pm3");
    let loaded = fixture.store.load().await.expect("should load");
    assert_eq!(loaded.boot, None);
}

#[tokio::test]
async fn a_snapshot_of_a_missing_dump_is_empty() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let snapshot = dump_snapshot(&dir.path().join("dump.yaml"))
        .await
        .expect("a missing dump is not an error");
    assert!(snapshot.is_empty());
}

#[tokio::test]
async fn a_snapshot_reports_a_corrupt_dump() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("dump.yaml");
    std::fs::write(&path, "not: [yaml").expect("write corrupt dump");
    let error = dump_snapshot(&path).await.unwrap_err();
    assert!(error.to_string().contains("cannot read"), "got: {error}");
}

#[tokio::test]
async fn a_snapshot_reads_names_and_pids_without_resolving_specs() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("dump.yaml");
    let body = "services:\n\
                - name: api\n\
                \x20 runtime:\n\
                \x20   pm_id: 0\n\
                \x20   status: online\n\
                \x20   restart_time: 0\n\
                \x20   unstable_restarts: 0\n\
                \x20   created_at_ms: 1\n\
                \x20   pid: 4242\n\
                \x20   started_at_ms: 2\n\
                - name: web\n\
                \x20 runtime:\n\
                \x20   pm_id: 1\n\
                \x20   status: stopped\n\
                \x20   restart_time: 0\n\
                \x20   unstable_restarts: 0\n\
                \x20   created_at_ms: 1\n\
                \x20   pid: null\n\
                \x20   started_at_ms: null\n";
    std::fs::write(&path, body).expect("write dump");
    let snapshot = dump_snapshot(&path).await.expect("the dump should parse");
    assert_eq!(
        snapshot,
        vec![
            usecases::ServiceSnapshot {
                name: "api".to_string(),
                pid: Some(4242),
            },
            usecases::ServiceSnapshot {
                name: "web".to_string(),
                pid: None,
            },
        ]
    );
}

#[tokio::test]
async fn a_snapshot_reports_a_dump_it_cannot_read() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let error = dump_snapshot(dir.path()).await.unwrap_err();
    assert!(error.to_string().contains("cannot read"), "got: {error}");
}
