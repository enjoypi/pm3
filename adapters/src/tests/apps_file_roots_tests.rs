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
