use std::collections::BTreeMap;

use usecases::AppSpec;

use crate::program::SVC_CWD_PLACEHOLDER;

pub async fn materialise_workspace(spec: &mut AppSpec) {
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
        *arg = expand_svc_cwd(arg, &spec.cwd);
    }
    let mut resolved = BTreeMap::from([(declared_cwd, spec.cwd.clone())]);
    for root in spec
        .sandbox
        .writable_roots
        .iter_mut()
        .chain(&mut spec.sandbox.derived_roots)
    {
        if let Some(known) = resolved.get(root.as_str()) {
            root.clone_from(known);
            continue;
        }
        let real = real_path(root).await;
        resolved.insert(root.clone(), real.clone());
        *root = real;
    }
}

#[must_use]
pub fn expand_svc_cwd(value: &str, cwd: &str) -> String {
    value.replace(SVC_CWD_PLACEHOLDER, cwd)
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
