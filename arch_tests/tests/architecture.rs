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
fn frameworks_must_not_depend_on_usecases() {
    assert_no_dependency("frameworks/Cargo.toml", &["usecases"]);
}

#[test]
fn frameworks_must_not_depend_on_entities() {
    assert_no_dependency("frameworks/Cargo.toml", &["entities"]);
}

#[test]
fn frameworks_must_not_depend_on_serde_json_at_runtime() {
    assert_no_runtime_dependency("frameworks/Cargo.toml", &["serde_json"]);
}

#[test]
fn entities_must_not_depend_on_serde() {
    assert_no_dependency("entities/Cargo.toml", &["serde"]);
}

#[test]
fn entities_must_not_depend_on_tokio() {
    assert_no_dependency("entities/Cargo.toml", &["tokio"]);
}

#[test]
fn usecases_must_not_depend_on_tokio_at_runtime() {
    assert_no_runtime_dependency("usecases/Cargo.toml", &["tokio"]);
}

#[test]
fn usecases_must_not_depend_on_axum() {
    assert_no_dependency("usecases/Cargo.toml", &["axum"]);
}

#[test]
fn entities_must_not_depend_on_usecases() {
    assert_no_dependency("entities/Cargo.toml", &["usecases"]);
}

#[test]
fn entities_must_not_depend_on_adapters() {
    assert_no_dependency("entities/Cargo.toml", &["adapters"]);
}

#[test]
fn entities_must_not_depend_on_frameworks() {
    assert_no_dependency("entities/Cargo.toml", &["frameworks"]);
}

#[test]
fn usecases_must_not_depend_on_adapters() {
    assert_no_dependency("usecases/Cargo.toml", &["adapters"]);
}

#[test]
fn usecases_must_not_depend_on_frameworks() {
    assert_no_dependency("usecases/Cargo.toml", &["frameworks"]);
}

#[test]
fn adapters_must_not_depend_on_frameworks() {
    assert_no_dependency("adapters/Cargo.toml", &["frameworks"]);
}

#[test]
fn entities_must_not_depend_on_serde_yaml2() {
    assert_no_dependency("entities/Cargo.toml", &["serde_yaml2"]);
}

#[test]
fn entities_must_not_depend_on_serde_json() {
    assert_no_dependency("entities/Cargo.toml", &["serde_json"]);
}

#[test]
fn usecases_must_not_depend_on_serde_yaml2() {
    assert_no_dependency("usecases/Cargo.toml", &["serde_yaml2"]);
}

#[test]
fn usecases_must_not_depend_on_serde_json() {
    assert_no_dependency("usecases/Cargo.toml", &["serde_json"]);
}

#[test]
fn usecases_must_not_depend_on_serde() {
    assert_no_dependency("usecases/Cargo.toml", &["serde"]);
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
