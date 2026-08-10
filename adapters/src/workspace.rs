use std::collections::BTreeMap;

use usecases::{AppSpec, PolicyError, validate_policy};

use crate::program::SERVICE_CWD_PLACEHOLDER;

pub async fn materialise_workspace(spec: &mut AppSpec) -> Result<(), PolicyError> {
    let declared_cwd = spec.cwd.clone();
    if let Err(error) = tokio::fs::create_dir_all(&declared_cwd).await {
        let reason = error.to_string();
        let path = declared_cwd.as_str();
        tracing::warn!(
            feature = "lifecycle",
            path,
            reason,
            action = "workspace",
            "cannot create the working directory"
        );
    }
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
        let real = resolve_cached(root, &mut resolved).await;
        if &real != root && !spec.sandbox.derived_roots.contains(&real) {
            spec.sandbox.derived_roots.push(real);
        }
    }
    for root in &mut spec.sandbox.unreadable_roots {
        *root = resolve_cached(root, &mut resolved).await;
    }
    validate_policy(&spec.sandbox)
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
