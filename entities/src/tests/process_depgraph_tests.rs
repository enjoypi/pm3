use super::*;

fn deps(entries: &[(&str, &[&str])]) -> Vec<(String, Vec<String>)> {
    entries
        .iter()
        .map(|(name, upstreams)| {
            (
                (*name).to_string(),
                upstreams.iter().map(|u| (*u).to_string()).collect(),
            )
        })
        .collect()
}

fn nodes(owned: &[(String, Vec<String>)]) -> Vec<DependencyNode<'_>> {
    owned
        .iter()
        .map(|(name, depends_on)| DependencyNode {
            name: name.as_str(),
            depends_on: depends_on.as_slice(),
        })
        .collect()
}

fn sort(entries: &[(&str, &[&str])]) -> Result<Vec<String>, DependencyError> {
    let owned = deps(entries);
    topo_sort(&nodes(&owned))
}

#[test]
fn empty_graph_sorts_to_empty_order() {
    let order = sort(&[]).expect("empty graph is acyclic");
    assert!(order.is_empty(), "got: {order:?}");
}

#[test]
fn independent_apps_keep_declaration_order() {
    let order = sort(&[("a", &[]), ("b", &[]), ("c", &[])]).expect("no dependencies");
    assert_eq!(order, vec!["a", "b", "c"]);
}

#[test]
fn linear_chain_starts_with_the_deepest_dependency() {
    let order =
        sort(&[("web", &["api"]), ("api", &["db"]), ("db", &[])]).expect("linear chain is acyclic");
    assert_eq!(order, vec!["db", "api", "web"]);
}

#[test]
fn diamond_dependency_places_root_first_and_sink_last() {
    let order = sort(&[
        ("sink", &["left", "right"]),
        ("left", &["root"]),
        ("right", &["root"]),
        ("root", &[]),
    ])
    .expect("diamond is acyclic");
    assert_eq!(order.first().map(String::as_str), Some("root"));
    assert_eq!(order.last().map(String::as_str), Some("sink"));
    assert_eq!(order.len(), 4);
}

#[test]
fn dependency_declared_twice_is_tolerated() {
    let order = sort(&[("api", &["db", "db"]), ("db", &[])]).expect("duplicate edge is harmless");
    assert_eq!(order, vec!["db", "api"]);
}

#[test]
fn unknown_dependency_is_rejected() {
    let err = sort(&[("api", &["ghost"])]).unwrap_err();
    assert_eq!(
        err,
        DependencyError::UnknownDependency {
            app: "api".to_string(),
            dependency: "ghost".to_string(),
        }
    );
}

#[test]
fn two_node_cycle_is_rejected() {
    let err = sort(&[("a", &["b"]), ("b", &["a"])]).unwrap_err();
    let DependencyError::Cycle { mut involved } = err else {
        panic!("expected a cycle error, got: {err}");
    };
    involved.sort();
    assert_eq!(involved, vec!["a", "b"]);
}

#[test]
fn three_node_cycle_is_rejected() {
    let err = sort(&[("a", &["c"]), ("b", &["a"]), ("c", &["b"])]).unwrap_err();
    let DependencyError::Cycle { mut involved } = err else {
        panic!("expected a cycle error, got: {err}");
    };
    involved.sort();
    assert_eq!(involved, vec!["a", "b", "c"]);
}

#[test]
fn self_cycle_is_reported_as_a_cycle() {
    let err = sort(&[("a", &["a"])]).unwrap_err();
    assert_eq!(
        err,
        DependencyError::Cycle {
            involved: vec!["a".to_string()],
        }
    );
}

#[test]
fn apps_merely_blocked_by_a_cycle_are_not_reported_as_involved() {
    let err = sort(&[("a", &["b"]), ("b", &["a"]), ("c", &["a"])]).unwrap_err();
    let DependencyError::Cycle { mut involved } = err else {
        panic!("expected a cycle error, got: {err}");
    };
    involved.sort();
    assert_eq!(involved, vec!["a", "b"]);
}

#[test]
fn downstream_of_blocked_apps_stays_out_of_the_cycle_report() {
    let err = sort(&[("a", &["b"]), ("b", &["a"]), ("c", &["a"]), ("d", &["c"])]).unwrap_err();
    let DependencyError::Cycle { mut involved } = err else {
        panic!("expected a cycle error, got: {err}");
    };
    involved.sort();
    assert_eq!(involved, vec!["a", "b"]);
}

#[test]
fn cycle_alongside_acyclic_component_is_still_rejected() {
    let err = sort(&[("ok", &[]), ("a", &["b"]), ("b", &["a"])]).unwrap_err();
    let DependencyError::Cycle { involved } = err else {
        panic!("expected a cycle error, got: {err}");
    };
    assert!(!involved.contains(&"ok".to_string()), "got: {involved:?}");
}

#[test]
fn every_dependency_error_renders_a_message() {
    let unknown = DependencyError::UnknownDependency {
        app: "api".to_string(),
        dependency: "ghost".to_string(),
    };
    assert_eq!(
        unknown.to_string(),
        "cannot resolve dependency 'ghost' declared by app 'api': no such app"
    );
    let cycle = DependencyError::Cycle {
        involved: vec!["a".to_string(), "b".to_string()],
    };
    assert_eq!(
        cycle.to_string(),
        "cannot order apps: dependency cycle involves a, b"
    );
}
