use entities::ProcessStatus;

use super::*;
use crate::{
    SignalScope,
    ports_test_helpers::{FakePorts, spec, started_table},
};

#[tokio::test]
async fn signalling_a_running_app_delivers_to_its_process_group() {
    let ports = FakePorts::new(1000);
    let mut table = started_table(&ports).await;
    let outcome = signal_app(&mut table, &AppSelector::Id(0), "hup", &ports)
        .await
        .expect("should signal");
    assert_eq!(
        outcome,
        SignalOutcome {
            name: "api".to_string(),
            signal: "HUP".to_string(),
        }
    );
    assert_eq!(ports.delivered(), vec![("HUP".to_string(), 100)]);
    assert_eq!(
        ports.signal_scopes(),
        vec![(100, SignalScope::ProcessGroup)]
    );
}

#[tokio::test]
async fn signalling_an_unknown_selector_reports_not_found() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    let err = signal_app(&mut table, &AppSelector::Id(9), "HUP", &ports)
        .await
        .unwrap_err();
    assert!(matches!(err, UsecaseError::NotFound(_)), "got: {err}");
}

#[tokio::test]
async fn signalling_a_settled_app_reports_not_running() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    table.upsert(spec("api"), 1000);
    let err = signal_app(&mut table, &AppSelector::Id(0), "HUP", &ports)
        .await
        .unwrap_err();
    assert!(matches!(err, UsecaseError::NotRunning(_)), "got: {err}");
    assert!(ports.delivered().is_empty());
}

#[tokio::test]
async fn signalling_reports_not_running_when_the_pid_is_gone() {
    let ports = FakePorts::new(1000);
    let mut table = started_table(&ports).await;
    ports.kill_silently(100);
    let err = signal_app(&mut table, &AppSelector::Id(0), "HUP", &ports)
        .await
        .unwrap_err();
    assert!(matches!(err, UsecaseError::NotRunning(_)), "got: {err}");
    assert!(ports.delivered().is_empty());
}

#[tokio::test]
async fn signalling_reports_not_running_when_the_pid_is_unreadable() {
    let ports = FakePorts::new(1000);
    let mut table = started_table(&ports).await;
    ports.break_probe_for(100);
    let err = signal_app(&mut table, &AppSelector::Id(0), "HUP", &ports)
        .await
        .unwrap_err();
    assert!(matches!(err, UsecaseError::NotRunning(_)), "got: {err}");
    assert!(ports.delivered().is_empty());
}

#[tokio::test]
async fn signalling_reports_not_running_when_the_pid_was_recycled() {
    let ports = FakePorts::new(1000);
    let mut table = started_table(&ports).await;
    ports.seed_live(100, "somebody-elses-process");
    let err = signal_app(&mut table, &AppSelector::Id(0), "HUP", &ports)
        .await
        .unwrap_err();
    assert!(matches!(err, UsecaseError::NotRunning(_)), "got: {err}");
    assert!(ports.delivered().is_empty());
}

#[tokio::test]
async fn an_unknown_signal_name_is_rejected() {
    let ports = FakePorts::new(1000);
    let mut table = started_table(&ports).await;
    let err = signal_app(&mut table, &AppSelector::Id(0), "KILL9", &ports)
        .await
        .unwrap_err();
    assert!(matches!(err, UsecaseError::InvalidSignal(_)), "got: {err}");
    assert!(ports.delivered().is_empty());
}

#[tokio::test]
async fn a_delivery_failure_propagates() {
    let ports = FakePorts::new(1000);
    let mut table = started_table(&ports).await;
    ports.fail_signal_for(100);
    let err = signal_app(&mut table, &AppSelector::Id(0), "HUP", &ports)
        .await
        .unwrap_err();
    assert!(matches!(err, UsecaseError::Signal(_)), "got: {err}");
}

#[tokio::test]
async fn a_service_mid_stop_can_still_be_signalled() {
    let ports = FakePorts::new(1000);
    let mut table = started_table(&ports).await;
    let stopping = table.find_mut(&AppSelector::Id(0)).expect("record present");
    stopping.runtime.mark_stopping();
    let outcome = signal_app(&mut table, &AppSelector::Id(0), "USR2", &ports)
        .await
        .expect("a stopping service still has a live pid");
    assert_eq!(outcome.signal, "USR2");
    let record = table.find(&AppSelector::Id(0)).expect("record present");
    assert_eq!(record.runtime.status, ProcessStatus::Stopping);
}
