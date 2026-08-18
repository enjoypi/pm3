use std::collections::BTreeMap;

use usecases::{AppSpec, PolicyError, root_is_forbidden, validate_policy};

use crate::program::SERVICE_CWD_PLACEHOLDER;

pub async fn materialise_workspace(
    spec: &mut AppSpec,
    forbidden_writable_roots: &[String],
) -> Result<(), PolicyError> {
    let declared_cwd = spec.cwd.clone();
    ensure_directory(&declared_cwd).await;
    spec.cwd = real_path(&declared_cwd).await;
    for arg in &mut spec.args {
        *arg = expand_service_cwd(arg, &spec.cwd);
    }
    let mut resolved = BTreeMap::from([(declared_cwd, spec.cwd.clone())]);
    for root in &mut spec.sandbox.derived_roots {
        *root = resolve_cached(root, &mut resolved).await;
    }
    let declared = spec.sandbox.writable_roots.clone();
    for root in &declared {
        ensure_directory(root).await;
        let real = resolve_cached(root, &mut resolved).await;
        if root_is_forbidden(forbidden_writable_roots, &real) {
            return Err(PolicyError::ForbiddenWritableRoot(root.clone()));
        }
        if &real != root && !spec.sandbox.derived_roots.contains(&real) {
            spec.sandbox.derived_roots.push(real);
        }
    }
    derive_readable_paths(spec, &mut resolved).await;
    for root in &mut spec.sandbox.unreadable_roots {
        *root = resolve_cached(root, &mut resolved).await;
    }
    validate_policy(&spec.sandbox)
}

async fn ensure_directory(path: &str) {
    let Err(error) = tokio::fs::create_dir_all(path).await else {
        return;
    };
    let reason = error.to_string();
    tracing::warn!(
        feature = "lifecycle",
        path,
        reason,
        action = "workspace",
        "cannot create a directory the sandbox has to grant"
    );
}

async fn derive_readable_paths(spec: &mut AppSpec, resolved: &mut BTreeMap<String, String>) {
    let declared: Vec<String> = spec
        .sandbox
        .readable_roots
        .iter()
        .cloned()
        .chain(std::iter::once(spec.script.clone()))
        .collect();
    for root in &declared {
        let real = resolve_cached(root, resolved).await;
        if &real != root && !spec.sandbox.derived_readable_roots.contains(&real) {
            spec.sandbox.derived_readable_roots.push(real);
        }
    }
}

async fn resolve_cached(path: &str, resolved: &mut BTreeMap<String, String>) -> String {
    if let Some(known) = resolved.get(path) {
        return known.clone();
    }
    let real = real_path(path).await;
    resolved.insert(path.to_string(), real.clone());
    real
}

#[must_use]
pub fn expand_service_cwd(value: &str, cwd: &str) -> String {
    value.replace(SERVICE_CWD_PLACEHOLDER, cwd)
}

async fn real_path(path: &str) -> String {
    tokio::fs::canonicalize(path).await.map_or_else(
        |_unresolved| path.to_string(),
        |resolved| resolved.to_string_lossy().into_owned(),
    )
}

#[cfg(test)]
#[path = "tests/workspace_tests.rs"]
mod tests;
