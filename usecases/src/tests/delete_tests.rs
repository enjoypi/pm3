use super::*;
use crate::{
    ports_test_helpers::{FakePorts, LOGS_DIR, spec, spec_with_deps},
    start::start_apps,
};

#[tokio::test]
async fn deleting_a_running_app_terminates_it_and_drops_the_record() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    start_apps(&mut table, &[spec("api")], LOGS_DIR, &ports).await;
    let outcome = delete_app(&mut table, &AppSelector::Id(0), &ports)
        .await
        .expect("delete should succeed");
    assert_eq!(outcome.name, "api");
    assert_eq!(outcome.force_kill_pid, Some(100));
    assert_eq!(ports.terminated(), vec![100]);
    assert!(table.records().is_empty());
}

#[tokio::test]
async fn deleting_a_stopped_app_needs_no_signal() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    table.upsert(spec("api"), 1000);
    let outcome = delete_app(&mut table, &AppSelector::Name("api".to_string()), &ports)
        .await
        .expect("delete should succeed");
    assert_eq!(outcome.force_kill_pid, None);
    assert!(table.records().is_empty());
}

#[tokio::test]
async fn deleting_an_unknown_selector_reports_not_found() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    let err = delete_app(&mut table, &AppSelector::Id(9), &ports)
        .await
        .unwrap_err();
    assert!(matches!(err, UsecaseError::NotFound(_)), "got: {err}");
}

#[tokio::test]
async fn deleting_a_service_others_depend_on_is_refused() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    table.upsert(spec("api"), 1000);
    table.upsert(spec_with_deps("web", &["api"]), 1000);
    let err = delete_app(&mut table, &AppSelector::Name("api".to_string()), &ports)
        .await
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "cannot delete app 'api': web still depends on it"
    );
}

#[tokio::test]
async fn a_service_others_depend_on_stays_in_the_table() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    table.upsert(spec("api"), 1000);
    table.upsert(spec_with_deps("web", &["api"]), 1000);
    delete_app(&mut table, &AppSelector::Name("api".to_string()), &ports)
        .await
        .expect_err("a dangling depends_on would break the next recovery");
    assert_eq!(table.records().len(), 2);
}

#[tokio::test]
async fn deleting_the_dependent_first_then_its_dependency_works() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    table.upsert(spec("api"), 1000);
    table.upsert(spec_with_deps("web", &["api"]), 1000);
    delete_app(&mut table, &AppSelector::Name("web".to_string()), &ports)
        .await
        .expect("nothing depends on web");
    delete_app(&mut table, &AppSelector::Name("api".to_string()), &ports)
        .await
        .expect("api is free once web is gone");
    assert!(table.records().is_empty());
}

#[tokio::test]
async fn a_persistence_failure_propagates() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    table.upsert(spec("api"), 1000);
    ports.fail_save();
    let err = delete_app(&mut table, &AppSelector::Id(0), &ports)
        .await
        .unwrap_err();
    assert!(matches!(err, UsecaseError::Dump(_)), "got: {err}");
}

#[tokio::test]
async fn a_signal_failure_propagates() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    start_apps(&mut table, &[spec("api")], LOGS_DIR, &ports).await;
    ports.fail_signal_for(100);
    let err = delete_app(&mut table, &AppSelector::Id(0), &ports)
        .await
        .unwrap_err();
    assert!(matches!(err, UsecaseError::Signal(_)), "got: {err}");
}

#[tokio::test]
async fn deleting_persists_the_shrunken_table() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    table.upsert(spec("api"), 1000);
    delete_app(&mut table, &AppSelector::Id(0), &ports)
        .await
        .expect("delete should succeed");
    assert!(ports.stored().is_empty());
}
