use entities::{AppSpec, ProcessIdentity, ProcessRuntime, ProcessStatus};

use super::*;
use crate::{ports_test_helpers::spec, record::ProcessRecord};

fn plain(name: &str, pm_id: u32) -> ProcessRecord {
    ProcessRecord {
        spec: spec(name),
        runtime: ProcessRuntime::new(pm_id, name.to_string(), 1000),
    }
}

fn armed(name: &str, pm_id: u32, cron: &str) -> ProcessRecord {
    let mut record = ProcessRecord {
        spec: AppSpec {
            schedule: Some(cron.to_string()),
            ..spec(name)
        },
        runtime: ProcessRuntime::new(pm_id, name.to_string(), 1000),
    };
    record.runtime.schedule_armed = true;
    record
}

fn launched(name: &str, pm_id: u32, pid: u32, token: &str) -> ProcessRecord {
    let mut record = plain(name, pm_id);
    record.runtime.pid = Some(pid);
    record.runtime.status = ProcessStatus::Online;
    record.runtime.identity = Some(ProcessIdentity {
        token: token.to_string(),
        launch_digest: "launch".to_string(),
        binary_digest: "binary".to_string(),
    });
    record
}

#[test]
fn an_empty_table_preserves_no_pid() {
    assert!(running_pids(&ProcessTable::new()).is_empty());
}

#[test]
fn only_the_running_records_have_their_pid_preserved() {
    let mut stopped = launched("web", 1, 200, "t2");
    stopped.runtime.status = ProcessStatus::Stopped;
    let table = ProcessTable::from_records(vec![launched("api", 0, 100, "t1"), stopped]);
    assert_eq!(running_pids(&table), vec![100]);
}

#[test]
fn a_running_record_without_a_pid_is_not_preserved() {
    let mut headless = launched("api", 0, 100, "t1");
    headless.runtime.pid = None;
    let table = ProcessTable::from_records(vec![headless]);
    assert!(running_pids(&table).is_empty());
}

#[test]
fn a_settled_table_reports_no_survivor() {
    let table = ProcessTable::from_records(vec![plain("api", 0)]);
    assert_eq!(unsettled_count(&table), 0);
}

#[test]
fn a_record_that_is_still_stopping_counts_as_a_survivor() {
    let mut stopping = launched("api", 0, 100, "t1");
    stopping.runtime.status = ProcessStatus::Stopping;
    let table = ProcessTable::from_records(vec![stopping, plain("web", 1)]);
    assert_eq!(unsettled_count(&table), 1);
}

#[test]
fn a_scheduled_and_armed_record_is_worth_arming_again() {
    let table = ProcessTable::from_records(vec![armed("nightly", 0, "0 3 * * *")]);
    assert_eq!(armed_schedule_names(&table), vec!["nightly".to_string()]);
}

#[test]
fn a_scheduled_record_the_operator_stopped_is_not_armed_again() {
    let mut disarmed = armed("nightly", 0, "0 3 * * *");
    disarmed.runtime.schedule_armed = false;
    let table = ProcessTable::from_records(vec![disarmed]);
    assert!(armed_schedule_names(&table).is_empty());
}

#[test]
fn a_record_without_a_schedule_is_never_armed() {
    let mut armless = plain("api", 0);
    armless.runtime.schedule_armed = true;
    let table = ProcessTable::from_records(vec![armless]);
    assert!(armed_schedule_names(&table).is_empty());
}

#[test]
fn a_schedule_is_read_back_from_its_record() {
    let table = ProcessTable::from_records(vec![armed("nightly", 0, "0 3 * * *")]);
    assert_eq!(schedule_of(&table, "nightly").as_deref(), Some("0 3 * * *"));
}

#[test]
fn an_unknown_app_has_no_schedule() {
    assert!(schedule_of(&ProcessTable::new(), "ghost").is_none());
}

#[test]
fn an_identity_token_is_read_back_from_its_record() {
    let table = ProcessTable::from_records(vec![launched("api", 0, 100, "Mon Jan  1 00:00:00")]);
    let token = identity_token_of(&table, &AppSelector::Name("api".to_string()));
    assert_eq!(token.as_deref(), Some("Mon Jan  1 00:00:00"));
}

#[test]
fn a_record_that_never_launched_has_no_identity_token() {
    let table = ProcessTable::from_records(vec![plain("api", 0)]);
    assert!(identity_token_of(&table, &AppSelector::Id(0)).is_none());
}

#[test]
fn an_unknown_app_has_no_identity_token() {
    assert!(identity_token_of(&ProcessTable::new(), &AppSelector::Id(7)).is_none());
}

#[test]
fn a_tracked_pid_is_traced_back_to_its_owner() {
    let table = ProcessTable::from_records(vec![launched("api", 0, 100, "t1")]);
    let (name, token) = owner_of_pid(&table, 100);
    assert_eq!(name, "api");
    assert_eq!(token.as_deref(), Some("t1"));
}

#[test]
fn an_owner_that_never_launched_yields_no_token() {
    let mut tracked = launched("api", 0, 100, "t1");
    tracked.runtime.identity = None;
    let table = ProcessTable::from_records(vec![tracked]);
    let (name, token) = owner_of_pid(&table, 100);
    assert_eq!(name, "api");
    assert!(token.is_none());
}

#[test]
fn a_pid_no_record_owns_is_labelled_a_stray() {
    let (name, token) = owner_of_pid(&ProcessTable::new(), 4321);
    assert_eq!(name, "stray-4321");
    assert!(token.is_none());
}

#[test]
fn a_tracked_pid_already_scheduled_for_a_kill_is_not_swept_again() {
    assert!(unswept_pids(&[100, 200], &[100, 200]).is_empty());
}

#[test]
fn a_tracked_pid_no_kill_covers_is_swept() {
    assert_eq!(unswept_pids(&[100, 200], &[100]), vec![200]);
}
