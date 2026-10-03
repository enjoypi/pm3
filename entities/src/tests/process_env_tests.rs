use super::*;

const ALL_SCOPES: [EnvScope; 3] = [EnvScope::Injected, EnvScope::Global, EnvScope::App];

#[test]
fn every_environment_scope_renders_its_label() {
    let labels: Vec<&str> = ALL_SCOPES.iter().map(|scope| scope.as_str()).collect();
    assert_eq!(labels, ["pm3", "global", "app"], "got: {labels:?}");
}

#[test]
fn an_app_value_wins_over_a_global_value_with_the_same_key() {
    let global = [EnvValue::new("PORT", "8080", EnvScope::Global)];
    let app = [EnvValue::new("PORT", "9090", EnvScope::App)];
    let merged = merge_environment(&[&global, &app]);
    assert_eq!(
        merged,
        vec![EnvValue::new("PORT", "9090", EnvScope::App)],
        "got: {merged:?}"
    );
}

#[test]
fn a_global_value_wins_over_an_injected_one() {
    let injected = [EnvValue::injected("HOME", "/root")];
    let global = [EnvValue::new("HOME", "/home/dev", EnvScope::Global)];
    let merged = merge_environment(&[&injected, &global]);
    assert_eq!(
        merged,
        vec![EnvValue::new("HOME", "/home/dev", EnvScope::Global)],
        "got: {merged:?}"
    );
}

#[test]
fn merging_keeps_every_distinct_key_in_ascending_order() {
    let global = [EnvValue::new("TZ", "UTC", EnvScope::Global)];
    let app = [EnvValue::new("PORT", "8080", EnvScope::App)];
    let merged = merge_environment(&[&global, &app]);
    let keys: Vec<&str> = merged.iter().map(|entry| entry.key.as_str()).collect();
    assert_eq!(keys, ["PORT", "TZ"], "got: {keys:?}");
}

#[test]
fn merging_no_layer_yields_nothing() {
    let merged = merge_environment(&[]);
    assert_eq!(merged, [], "got: {merged:?}");
}

#[test]
fn an_empty_layer_leaves_the_others_alone() {
    let app = [EnvValue::new("PORT", "8080", EnvScope::App)];
    let merged = merge_environment(&[&[], &app]);
    assert_eq!(
        merged,
        vec![EnvValue::new("PORT", "8080", EnvScope::App)],
        "got: {merged:?}"
    );
}
