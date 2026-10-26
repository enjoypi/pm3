#![allow(
    clippy::tests_outside_test_module,
    reason = "integration tests in tests/ are inherently outside #[cfg(test)]"
)]

use std::{collections::HashSet, path::Path};

fn workspace_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("arch_tests should be inside workspace")
        .to_path_buf()
}

const ALL_DEPENDENCY_TABLES: &[&str] = &["dependencies", "dev-dependencies", "build-dependencies"];
const RUNTIME_DEPENDENCY_TABLES: &[&str] = &["dependencies"];

fn read_dependency_names(cargo_toml_path: &str, tables: &[&str]) -> HashSet<String> {
    let full_path = workspace_root().join(cargo_toml_path);
    let content = std::fs::read_to_string(&full_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", full_path.display()));
    dependency_names(&content, tables)
        .unwrap_or_else(|e| panic!("failed to parse {cargo_toml_path}: {e}"))
}

fn dependency_names(cargo_toml: &str, tables: &[&str]) -> Result<HashSet<String>, toml::de::Error> {
    let table: toml::Table = cargo_toml.parse()?;
    let mut deps = HashSet::new();

    for &dependency_table in tables {
        if let Some(dependencies) = table.get(dependency_table).and_then(|v| v.as_table()) {
            for (alias, spec) in dependencies {
                deps.insert(alias.clone());
                if let Some(renamed) = spec.get("package").and_then(toml::Value::as_str) {
                    deps.insert(renamed.to_string());
                }
            }
        }
    }

    Ok(deps)
}

#[test]
fn dependency_names_expose_package_rename_behind_alias() {
    let names = dependency_names(
        "[dependencies]\nent = { package = \"entities\" }\n",
        ALL_DEPENDENCY_TABLES,
    )
    .expect("fixture parses");
    assert!(names.contains("entities"), "got: {names:?}");
}

#[test]
fn dependency_names_ignore_dev_table_when_scoped_to_runtime() {
    let names = dependency_names(
        "[dev-dependencies]\ntokio = \"1\"\n",
        RUNTIME_DEPENDENCY_TABLES,
    )
    .expect("fixture parses");
    assert!(names.is_empty(), "got: {names:?}");
}

fn assert_no_dependency(cargo_toml: &str, forbidden: &[&str]) {
    assert_forbidden(cargo_toml, forbidden, ALL_DEPENDENCY_TABLES);
}

fn assert_no_runtime_dependency(cargo_toml: &str, forbidden: &[&str]) {
    assert_forbidden(cargo_toml, forbidden, RUNTIME_DEPENDENCY_TABLES);
}

fn assert_forbidden(cargo_toml: &str, forbidden: &[&str], tables: &[&str]) {
    let crate_name = Path::new(cargo_toml)
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    let deps = read_dependency_names(cargo_toml, tables);

    for &forbidden_dep in forbidden {
        assert!(
            !deps.contains(forbidden_dep),
            "VIOLATION: {cargo_toml} depends on '{forbidden_dep}'. \
             {crate_name} must not directly depend on '{forbidden_dep}'. \
             Route access through the appropriate layer's re-exports instead."
        );
    }
}

#[test]
fn frameworks_reaches_inner_layers_only_through_adapters() {
    assert_no_dependency("frameworks/Cargo.toml", &["usecases", "entities"]);
}

#[test]
fn frameworks_must_not_depend_on_serde_json_at_runtime() {
    assert_no_runtime_dependency("frameworks/Cargo.toml", &["serde_json"]);
}

#[test]
fn entities_depend_on_no_outer_layer_or_serialization() {
    assert_no_dependency(
        "entities/Cargo.toml",
        &[
            "serde",
            "tokio",
            "usecases",
            "adapters",
            "frameworks",
            "serde_yaml2",
            "serde_json",
        ],
    );
}

#[test]
fn usecases_must_not_depend_on_tokio_at_runtime() {
    assert_no_runtime_dependency("usecases/Cargo.toml", &["tokio"]);
}

#[test]
fn usecases_depend_on_no_outer_layer_or_serialization() {
    assert_no_dependency(
        "usecases/Cargo.toml",
        &[
            "axum",
            "adapters",
            "frameworks",
            "serde_yaml2",
            "serde_json",
            "serde",
        ],
    );
}

#[test]
fn adapters_must_not_depend_on_frameworks() {
    assert_no_dependency("adapters/Cargo.toml", &["frameworks"]);
}

fn read_file_content(relative_path: &str) -> String {
    let full_path = workspace_root().join(relative_path);
    std::fs::read_to_string(&full_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", full_path.display()))
}

fn assert_no_wildcard_reexport(source_file: &str, upstream_crate: &str) {
    let content = read_file_content(source_file);
    let violation = wildcard_reexport_violation(&content, upstream_crate);

    assert!(
        violation.is_none(),
        "VIOLATION: {source_file} has wildcard/bare re-export of '{upstream_crate}': \
         `{}`. Re-exports must be specific types: `pub use {upstream_crate}::{{Type1, Type2}};`",
        violation.unwrap_or_default()
    );
}

fn wildcard_reexport_violation(content: &str, upstream_crate: &str) -> Option<String> {
    for statement in reexport_statements(content) {
        if !statement.contains(upstream_crate) {
            continue;
        }
        if statement.contains('*') || statement == format!("pub use {upstream_crate};") {
            return Some(statement);
        }
    }
    None
}

fn reexport_statements(content: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if current.is_empty() && !trimmed.starts_with("pub use") {
            continue;
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(trimmed);
        if trimmed.ends_with(';') {
            statements.push(std::mem::take(&mut current));
        }
    }

    statements
}

#[test]
fn wildcard_reexport_violation_catches_glob_mixed_with_named_items() {
    let violation = wildcard_reexport_violation("pub use usecases::{AppConfig, *};\n", "usecases");
    assert_eq!(
        violation.as_deref(),
        Some("pub use usecases::{AppConfig, *};")
    );
}

#[test]
fn wildcard_reexport_violation_catches_glob_split_over_lines() {
    let violation = wildcard_reexport_violation("pub use usecases::{\n    *,\n};\n", "usecases");
    assert_eq!(violation.as_deref(), Some("pub use usecases::{ *, };"));
}

#[test]
fn wildcard_reexport_violation_catches_bare_reexport() {
    let violation = wildcard_reexport_violation("pub use usecases;\n", "usecases");
    assert_eq!(violation.as_deref(), Some("pub use usecases;"));
}

#[test]
fn wildcard_reexport_violation_accepts_named_items() {
    let violation =
        wildcard_reexport_violation("pub use usecases::{AppConfig, ConfigError};\n", "usecases");
    assert_eq!(violation, None);
}

#[test]
fn usecases_reexports_entities_selectively() {
    assert_no_wildcard_reexport("usecases/src/lib.rs", "entities");
}

#[test]
fn adapters_reexports_usecases_selectively() {
    assert_no_wildcard_reexport("adapters/src/lib.rs", "usecases");
}

const FORBIDDEN_SOURCE_MARKERS: &[&str] = &["#![feature(", "coverage(off)", "coverage_nightly"];

fn workspace_members() -> Vec<String> {
    let table: toml::Table = read_file_content("Cargo.toml")
        .parse()
        .expect("workspace Cargo.toml parses");
    table["workspace"]["members"]
        .as_array()
        .expect("workspace.members is an array")
        .iter()
        .filter_map(toml::Value::as_str)
        .filter(|member| *member != "arch_tests")
        .map(str::to_string)
        .collect()
}

fn rust_sources(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir).expect("source dir is readable") {
        let path = entry.expect("dir entry is readable").path();
        if path.is_dir() {
            found.extend(rust_sources(&path));
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            found.push(path);
        }
    }
    found
}

fn forbidden_source_marker(content: &str) -> Option<&'static str> {
    let compact: String = content.chars().filter(|c| !c.is_whitespace()).collect();
    FORBIDDEN_SOURCE_MARKERS
        .iter()
        .copied()
        .find(|marker| compact.contains(marker))
}

fn declares_features(cargo_toml: &str) -> bool {
    let table: toml::Table = cargo_toml.parse().expect("Cargo.toml parses");
    table.contains_key("features")
}

#[test]
fn forbidden_source_marker_catches_a_spaced_coverage_off() {
    assert_eq!(
        forbidden_source_marker("#[coverage( off )]\nfn f() {}\n"),
        Some("coverage(off)")
    );
}

#[test]
fn forbidden_source_marker_catches_a_nightly_feature_gate() {
    assert_eq!(
        forbidden_source_marker("#![feature(coverage_attribute)]\n"),
        Some("#![feature(")
    );
}

#[test]
fn forbidden_source_marker_accepts_plain_code() {
    assert_eq!(forbidden_source_marker("fn covered() {}\n"), None);
}

#[test]
fn declares_features_spots_a_features_table() {
    assert!(declares_features("[features]\nextra = []\n"));
    assert!(!declares_features("[dependencies]\n"));
}

#[test]
fn no_member_opts_out_of_coverage_or_uses_nightly_features() {
    for member in workspace_members() {
        for path in rust_sources(&workspace_root().join(&member)) {
            let content = std::fs::read_to_string(&path).expect("source is readable");
            let marker = forbidden_source_marker(&content);
            assert!(
                marker.is_none(),
                "VIOLATION: {} uses `{}`. Coverage MUST NOT be switched off and nightly features MUST NOT be enabled: cover the code with a test or delete it",
                path.display(),
                marker.unwrap_or_default()
            );
        }
    }
}

#[test]
fn no_member_declares_cargo_features() {
    for member in workspace_members() {
        let manifest = format!("{member}/Cargo.toml");
        assert!(
            !declares_features(&read_file_content(&manifest)),
            "VIOLATION: {manifest} declares [features]. Every build MUST compile one code path so coverage sees all of it"
        );
    }
}
