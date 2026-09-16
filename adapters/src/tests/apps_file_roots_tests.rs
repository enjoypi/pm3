use super::*;

#[test]
fn dedup_keeps_the_first_occurrence_and_its_order() {
    let roots = dedup_roots(
        ["/srv/app", "/var/log", "/srv/app", "/tmp"]
            .into_iter()
            .map(ToString::to_string),
    );
    assert_eq!(roots, ["/srv/app", "/var/log", "/tmp"]);
}

#[test]
fn dedup_of_nothing_yields_nothing() {
    assert!(dedup_roots(Vec::new()).is_empty());
}

#[test]
fn a_root_nested_in_another_is_dropped() {
    let roots = dominant_roots(["/s/pm3".to_string(), "/s/pm3/run".to_string()]);
    assert_eq!(roots, vec!["/s/pm3".to_string()], "got: {roots:?}");
}

#[test]
fn sibling_roots_all_survive() {
    let roots = dominant_roots(["/c/pm3".to_string(), "/s/pm3".to_string()]);
    assert_eq!(
        roots,
        vec!["/c/pm3".to_string(), "/s/pm3".to_string()],
        "got: {roots:?}"
    );
}

#[test]
fn an_identical_root_is_kept_once() {
    let roots = dominant_roots(["/s/pm3".to_string(), "/s/pm3".to_string()]);
    assert_eq!(roots, vec!["/s/pm3".to_string()], "got: {roots:?}");
}
