use entities::topo_sort;

use super::{test_helpers::*, *};

#[test]
fn a_new_table_holds_no_records() {
    assert!(ProcessTable::new().records().is_empty());
}

#[test]
fn upsert_assigns_process_ids_from_zero_upwards() {
    let mut table = ProcessTable::new();
    assert_eq!(table.upsert(spec("api"), 1000), 0);
    assert_eq!(table.upsert(spec("web"), 1000), 1);
    assert_eq!(table.records().len(), 2);
}

#[test]
fn upsert_of_a_known_name_replaces_the_spec_and_keeps_the_id() {
    let mut table = ProcessTable::new();
    let first = table.upsert(spec("api"), 1000);
    let updated = AppSpec {
        script: "/usr/bin/false".to_string(),
        ..spec("api")
    };
    assert_eq!(table.upsert(updated, 2000), first);
    assert_eq!(table.records().len(), 1);
    let stored = table
        .find(&AppSelector::Name("api".to_string()))
        .expect("record present");
    assert_eq!(stored.spec.script, "/usr/bin/false");
}

#[test]
fn upsert_keeps_runtime_state_when_replacing_a_spec() {
    let mut table = ProcessTable::new();
    table.upsert(spec("api"), 1000);
    let selector = AppSelector::Name("api".to_string());
    table
        .find_mut(&selector)
        .expect("record present")
        .runtime
        .mark_launched(99, 1500);
    table.upsert(spec("api"), 2000);
    let stored = table.find(&selector).expect("record present");
    assert_eq!(stored.runtime.pid, Some(99));
}

#[test]
fn new_ids_continue_past_the_highest_existing_id() {
    let mut table = ProcessTable::from_records(Vec::new());
    table.upsert(spec("a"), 1000);
    table.upsert(spec("b"), 1000);
    table.remove(&AppSelector::Id(0));
    assert_eq!(table.upsert(spec("c"), 1000), 2);
}

#[test]
fn removing_the_highest_id_does_not_hand_it_out_again() {
    let mut table = ProcessTable::new();
    table.upsert(spec("a"), 1000);
    table.upsert(spec("b"), 1000);
    table.remove(&AppSelector::Id(1));
    assert_eq!(table.upsert(spec("c"), 1000), 2);
}

#[test]
fn a_table_restored_from_records_continues_past_their_highest_id() {
    let mut table = ProcessTable::from_records(vec![record_with_id("a", 4)]);
    assert_eq!(table.upsert(spec("b"), 1000), 5);
}

#[test]
fn find_locates_a_record_by_id() {
    let mut table = ProcessTable::new();
    table.upsert(spec("api"), 1000);
    let found = table.find(&AppSelector::Id(0)).expect("record present");
    assert_eq!(found.runtime.name, "api");
}

#[test]
fn find_returns_nothing_for_an_unknown_selector() {
    let table = ProcessTable::new();
    assert!(table.find(&AppSelector::Id(9)).is_none());
}

#[test]
fn find_mut_returns_nothing_for_an_unknown_selector() {
    let mut table = ProcessTable::new();
    assert!(table.find_mut(&AppSelector::Id(9)).is_none());
}

#[test]
fn find_by_name_mut_returns_nothing_for_an_unknown_name() {
    let mut table = ProcessTable::new();
    assert!(table.find_by_name_mut("ghost").is_none());
}

#[test]
fn remove_returns_the_dropped_record() {
    let mut table = ProcessTable::new();
    table.upsert(spec("api"), 1000);
    let removed = table
        .remove(&AppSelector::Name("api".to_string()))
        .expect("record present");
    assert_eq!(removed.runtime.name, "api");
    assert!(table.records().is_empty());
}

#[test]
fn remove_returns_nothing_for_an_unknown_selector() {
    let mut table = ProcessTable::new();
    assert!(table.remove(&AppSelector::Id(9)).is_none());
}

#[test]
fn dependency_nodes_feed_the_topological_sort() {
    let mut table = ProcessTable::new();
    table.upsert(spec_with_deps("web", &["api"]), 1000);
    table.upsert(spec("api"), 1000);
    let order = topo_sort(&table.dependency_nodes()).expect("acyclic");
    assert_eq!(order, vec!["api", "web"]);
}
