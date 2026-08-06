use usecases::ProcessStatus;

use super::*;
use crate::process_records::{
    CREATED_AT_MS, SAMPLE_BINARY_DIGEST, SAMPLE_LAUNCH_DIGEST, SAMPLE_PID, SAMPLE_TOKEN,
    STARTED_AT_MS, sample_identity, sample_record, stopped_record,
};

fn encoded(name: &str) -> StateDto {
    let mut doc = encode_states(&[sample_record(name)], None);
    doc.services.pop().expect("one encoded service")
}

fn decoded(dto: StateDto) -> ProcessRuntime {
    decode_state(dto).expect("should decode one service")
}

#[test]
fn encode_names_the_service_once() {
    let dto = encoded("web");
    assert_eq!(dto.name, "web");
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
fn encode_leaves_an_idle_service_without_a_pid() {
    let mut doc = encode_states(&[stopped_record("web")], None);
    let dto = doc.services.pop().expect("one encoded service");
    assert_eq!(dto.runtime.pid, None);
    assert_eq!(dto.runtime.started_at_ms, None);
}

#[test]
fn encode_carries_the_identity_of_the_running_process() {
    let dto = encoded("web");
    let identity = dto
        .runtime
        .identity
        .expect("a running service has an identity");
    assert_eq!(identity.token, SAMPLE_TOKEN);
    assert_eq!(identity.launch_digest, SAMPLE_LAUNCH_DIGEST);
    assert_eq!(identity.binary_digest, SAMPLE_BINARY_DIGEST);
}

#[test]
fn encode_leaves_an_idle_service_without_an_identity() {
    let mut doc = encode_states(&[stopped_record("web")], None);
    let dto = doc.services.pop().expect("one encoded service");
    assert!(dto.runtime.identity.is_none());
}

#[test]
fn an_idle_service_writes_no_identity_key_at_all() {
    let doc = encode_states(&[stopped_record("web")], None);
    let yaml = serde_yaml2::to_string(&doc).expect("the dump document serializes");
    assert!(!yaml.contains("identity"), "got: {yaml}");
}

#[test]
fn decode_restores_the_identity() {
    let runtime = decoded(encoded("web"));
    assert_eq!(runtime.identity, Some(sample_identity()));
}

#[test]
fn decode_accepts_a_dump_written_before_identities_existed() {
    let mut dto = encoded("web");
    dto.runtime.identity = None;
    let runtime = decode_state(dto).expect("an identity-less dump stays readable");
    assert_eq!(runtime.identity, None);
}

#[test]
fn encode_keeps_the_declared_order() {
    let doc = encode_states(&[sample_record("web"), sample_record("db")], None);
    let names: Vec<&str> = doc.services.iter().map(|dto| dto.name.as_str()).collect();
    assert_eq!(names, vec!["web", "db"]);
}

#[test]
fn decode_restores_the_runtime() {
    let runtime = decoded(encoded("web"));
    assert_eq!(runtime, sample_record("web").runtime);
}

#[test]
fn decode_names_the_runtime_after_the_service() {
    let runtime = decoded(encoded("web"));
    assert_eq!(runtime.name, "web");
}

#[test]
fn decode_clears_a_pending_restart_request() {
    let mut record = sample_record("web");
    record.runtime.request_restart();
    let mut doc = encode_states(&[record], None);
    let restored = decoded(doc.services.pop().expect("one encoded service"));
    assert!(!restored.pending_restart);
}

#[test]
fn decode_rejects_an_unknown_status() {
    let mut dto = encoded("web");
    dto.runtime.status = "zombie".to_string();
    let err = decode_state(dto).unwrap_err().to_string();
    assert_eq!(err, "cannot decode app 'web': unknown status 'zombie'");
}

#[test]
fn decode_rejects_a_running_status_without_a_pid() {
    let mut dto = encoded("web");
    dto.runtime.pid = None;
    let err = decode_state(dto).unwrap_err().to_string();
    assert_eq!(
        err,
        "cannot decode app 'web': cannot accept process 'web' marked 'online' without a pid"
    );
}

#[test]
fn an_encoded_document_carries_the_boot_it_was_written_under() {
    let doc = encode_states(&[sample_record("web")], Some("Tue Jul 28 14:06:28 2026"));
    assert_eq!(doc.boot.as_deref(), Some("Tue Jul 28 14:06:28 2026"));
}

#[test]
fn a_document_saved_without_a_known_boot_carries_none() {
    let doc = encode_states(&[sample_record("web")], None);
    assert_eq!(doc.boot, None);
}
