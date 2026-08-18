use super::*;

fn row(name: &str, pid: Option<u32>) -> ServiceSnapshot {
    ServiceSnapshot {
        name: name.to_owned(),
        pid,
    }
}

#[test]
fn a_service_missing_after_the_handover_is_lost() {
    let before = vec![row("api", Some(10))];
    let comparison = compare_handover(&before, &[]);
    assert_eq!(comparison.lost, vec!["api".to_owned()]);
    assert!(comparison.adopted.is_empty());
    assert!(comparison.restarted.is_empty());
}

#[test]
fn a_service_with_the_same_pid_is_adopted() {
    let before = vec![row("api", Some(10))];
    let after = vec![row("api", Some(10))];
    let comparison = compare_handover(&before, &after);
    assert_eq!(comparison.adopted, vec!["api".to_owned()]);
}

#[test]
fn a_service_with_a_new_pid_is_restarted() {
    let before = vec![row("api", Some(10))];
    let after = vec![row("api", Some(11))];
    let comparison = compare_handover(&before, &after);
    assert_eq!(comparison.restarted, vec!["api".to_owned()]);
}

#[test]
fn a_stopped_service_coming_back_online_is_restarted() {
    let before = vec![row("api", None)];
    let after = vec![row("api", Some(11))];
    let comparison = compare_handover(&before, &after);
    assert_eq!(comparison.restarted, vec!["api".to_owned()]);
}

#[test]
fn a_service_still_without_a_pid_is_not_a_change() {
    let before = vec![row("api", None)];
    let after = vec![row("api", None)];
    let comparison = compare_handover(&before, &after);
    assert_eq!(comparison, HandoverComparison::default());
}

#[test]
fn a_service_that_lost_its_pid_is_reported_lost() {
    let before = vec![row("api", Some(10))];
    let after = vec![row("api", None)];
    let comparison = compare_handover(&before, &after);
    assert_eq!(comparison.lost, vec!["api".to_string()]);
    assert!(comparison.adopted.is_empty() && comparison.restarted.is_empty());
}

#[test]
fn a_service_that_was_already_stopped_stays_out_of_the_comparison() {
    let before = vec![row("api", None)];
    let after = vec![row("api", None)];
    let comparison = compare_handover(&before, &after);
    assert_eq!(comparison, HandoverComparison::default());
}

#[test]
fn a_service_that_gained_a_pid_counts_as_restarted() {
    let before = vec![row("api", None)];
    let after = vec![row("api", Some(11))];
    let comparison = compare_handover(&before, &after);
    assert_eq!(comparison.restarted, vec!["api".to_string()]);
}

#[test]
fn services_new_to_the_after_side_are_ignored() {
    let comparison = compare_handover(&[], &[row("api", Some(10))]);
    assert_eq!(comparison, HandoverComparison::default());
}

#[test]
fn an_empty_comparison_says_there_is_nothing_to_reclaim() {
    assert_eq!(
        describe_handover(&HandoverComparison::default()),
        "no managed services to reclaim"
    );
}

#[test]
fn the_description_lists_each_non_empty_group() {
    let comparison = HandoverComparison {
        adopted: vec!["api".to_owned()],
        restarted: vec!["web".to_owned(), "worker".to_owned()],
        lost: vec!["db".to_owned()],
    };
    assert_eq!(
        describe_handover(&comparison),
        "adopted 1: api\nrestarted 2: web, worker\nlost 1: db"
    );
}

#[test]
fn empty_groups_are_left_out_of_the_description() {
    let comparison = HandoverComparison {
        adopted: vec!["api".to_owned()],
        restarted: vec![],
        lost: vec![],
    };
    assert_eq!(describe_handover(&comparison), "adopted 1: api");
}
