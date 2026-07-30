use entities::ProcessStatus;

use super::*;
use crate::ports_test_helpers::spec;

#[test]
fn listing_an_empty_table_yields_no_views() {
    assert!(list_apps(&ProcessTable::new(), 1000).is_empty());
}

#[test]
fn listing_projects_every_record() {
    let mut table = ProcessTable::new();
    table.upsert(spec("api"), 1000);
    table.upsert(spec("web"), 1000);
    let views = list_apps(&table, 2000);
    assert_eq!(views.len(), 2);
    assert_eq!(views[0].name, "api");
    assert_eq!(views[1].name, "web");
    assert_eq!(views[0].status, ProcessStatus::Stopped);
}

#[test]
fn describing_a_known_app_returns_its_view() {
    let mut table = ProcessTable::new();
    table.upsert(spec("api"), 1000);
    let view = describe_app(&table, &AppSelector::Id(0), 2000).expect("record present");
    assert_eq!(view.name, "api");
    assert_eq!(view.pm_id, 0);
}

#[test]
fn describing_an_unknown_app_reports_not_found() {
    let table = ProcessTable::new();
    let err = describe_app(&table, &AppSelector::Name("ghost".to_string()), 2000).unwrap_err();
    assert!(matches!(err, UsecaseError::NotFound(_)), "got: {err}");
}
