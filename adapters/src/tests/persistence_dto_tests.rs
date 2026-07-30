use usecases::{ProcessStatus, SandboxMode};

use super::*;
use crate::process_records::{
    CREATED_AT_MS, SAMPLE_PID, STARTED_AT_MS, sample_record, stopped_record,
};

fn encoded(name: &str) -> RecordDto {
    let mut doc = encode_records(&[sample_record(name)]);
    doc.apps.pop().expect("one encoded app")
}

fn decoded(dto: RecordDto) -> ProcessRecord {
    let mut records =
        decode_records(DumpDocument { apps: vec![dto] }).expect("should decode one app");
    records.pop().expect("one decoded app")
}

#[test]
fn encode_names_the_app_once() {
    let dto = encoded("web");
    assert_eq!(dto.name, "web");
}

#[test]
fn encode_carries_the_command_line() {
    let dto = encoded("web");
    assert_eq!(dto.script, "/usr/bin/node");
    assert_eq!(dto.args, vec!["server.js", "--port=8080"]);
    assert_eq!(dto.cwd, "/srv/web");
}

#[test]
fn encode_turns_the_environment_into_a_mapping() {
    let dto = encoded("web");
    assert_eq!(dto.env.get("PORT").map(String::as_str), Some("8080"));
}

#[test]
fn encode_carries_the_restart_policy() {
    let dto = encoded("web");
    assert!(dto.autorestart);
    assert_eq!(dto.min_uptime_ms, 1000);
    assert_eq!(dto.max_restarts, 15);
    assert_eq!(dto.restart_delay_ms, 40);
}

#[test]
fn encode_spells_the_sandbox_mode_out() {
    let dto = encoded("web");
    assert_eq!(dto.sandbox.mode, SandboxMode::WorkspaceWrite.as_str());
}

#[test]
fn encode_carries_the_sandbox_details() {
    let dto = encoded("web");
    assert!(!dto.sandbox.network);
    assert_eq!(dto.sandbox.writable_roots, vec!["/srv/web"]);
}

#[test]
fn encode_spells_the_status_out() {
    let dto = encoded("web");
    assert_eq!(dto.runtime.status, ProcessStatus::Online.as_str());
}

#[test]
fn encode_carries_the_runtime_counters() {
    let dto = encoded("web");
    assert_eq!(dto.runtime.pm_id, 3);
    assert_eq!(dto.runtime.pid, Some(SAMPLE_PID));
    assert_eq!(dto.runtime.restart_time, 2);
    assert_eq!(dto.runtime.unstable_restarts, 1);
}

#[test]
fn encode_carries_the_timestamps() {
    let dto = encoded("web");
    assert_eq!(dto.runtime.created_at_ms, CREATED_AT_MS);
    assert_eq!(dto.runtime.started_at_ms, Some(STARTED_AT_MS));
}

#[test]
fn encode_leaves_an_idle_app_without_a_pid() {
    let mut doc = encode_records(&[stopped_record("web")]);
    let dto = doc.apps.pop().expect("one encoded app");
    assert_eq!(dto.runtime.pid, None);
    assert_eq!(dto.runtime.started_at_ms, None);
}

#[test]
fn encode_keeps_the_declared_order() {
    let doc = encode_records(&[sample_record("web"), sample_record("db")]);
    let names: Vec<&str> = doc.apps.iter().map(|dto| dto.name.as_str()).collect();
    assert_eq!(names, vec!["web", "db"]);
}

#[test]
fn decode_restores_the_spec() {
    let record = decoded(encoded("web"));
    assert_eq!(record.spec, sample_record("web").spec);
}

#[test]
fn decode_restores_the_runtime() {
    let record = decoded(encoded("web"));
    assert_eq!(record.runtime, sample_record("web").runtime);
}

#[test]
fn decode_names_the_runtime_after_the_app() {
    let record = decoded(encoded("web"));
    assert_eq!(record.runtime.name, "web");
}

#[test]
fn decode_clears_a_pending_restart_request() {
    let mut record = sample_record("web");
    record.runtime.request_restart();
    let mut doc = encode_records(&[record]);
    let restored = decoded(doc.apps.pop().expect("one encoded app"));
    assert!(!restored.runtime.pending_restart);
}

#[test]
fn decode_rejects_an_unknown_status() {
    let mut dto = encoded("web");
    dto.runtime.status = "zombie".to_string();
    let err = decode_records(DumpDocument { apps: vec![dto] })
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("cannot decode app 'web': unknown status 'zombie'"),
        "got: {err}"
    );
}

#[test]
fn decode_rejects_an_unknown_sandbox_mode() {
    let mut dto = encoded("web");
    dto.sandbox.mode = "yolo".to_string();
    let err = decode_records(DumpDocument { apps: vec![dto] })
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("cannot decode app 'web': unknown sandbox mode 'yolo'"),
        "got: {err}"
    );
}

#[test]
fn decode_accepts_an_empty_document() {
    let records = decode_records(DumpDocument { apps: Vec::new() }).expect("should decode");
    assert!(records.is_empty(), "got: {records:?}");
}
