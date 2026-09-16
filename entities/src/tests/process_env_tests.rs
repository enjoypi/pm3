use super::*;

const ALL_SCOPES: [EnvScope; 3] = [EnvScope::Injected, EnvScope::Global, EnvScope::App];

#[test]
fn every_environment_scope_renders_its_label() {
    let labels: Vec<&str> = ALL_SCOPES.iter().map(|scope| scope.as_str()).collect();
    assert_eq!(labels, ["pm3", "global", "app"], "got: {labels:?}");
}

#[test]
fn an_app_value_wins_over_a_global_value_with_the_same_key() {
    let global = [EnvValue::global("PORT", "8080")];
    let app = [EnvValue::app("PORT", "9090")];
    let merged = merge_environment(&[&global, &app]);
    assert_eq!(
        merged,
        vec![EnvValue::app("PORT", "9090")],
        "got: {merged:?}"
    );
}

#[test]
fn a_global_value_wins_over_an_injected_one() {
    let injected = [EnvValue::injected("HOME", "/root")];
    let global = [EnvValue::global("HOME", "/home/dev")];
    let merged = merge_environment(&[&injected, &global]);
    assert_eq!(
        merged,
        vec![EnvValue::global("HOME", "/home/dev")],
        "got: {merged:?}"
    );
}

#[test]
fn merging_keeps_every_distinct_key_in_ascending_order() {
    let global = [EnvValue::global("TZ", "UTC")];
    let app = [EnvValue::app("PORT", "8080")];
    let merged = merge_environment(&[&global, &app]);
    let keys: Vec<&str> = merged.iter().map(|entry| entry.key.as_str()).collect();
    assert_eq!(keys, ["PORT", "TZ"], "got: {keys:?}");
}

#[test]
fn merging_no_layer_yields_nothing() {
    let merged = merge_environment(&[]);
    assert!(merged.is_empty(), "got: {merged:?}");
}

#[test]
fn an_empty_layer_leaves_the_others_alone() {
    let app = [EnvValue::app("PORT", "8080")];
    let merged = merge_environment(&[&[], &app]);
    assert_eq!(
        merged,
        vec![EnvValue::app("PORT", "8080")],
        "got: {merged:?}"
    );
}
