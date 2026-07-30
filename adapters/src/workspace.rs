use usecases::AppSpec;

use crate::program::SVC_CWD_PLACEHOLDER;

pub async fn materialise_workspace(spec: &mut AppSpec) {
    tokio::fs::create_dir_all(&spec.cwd).await.ok();
    spec.cwd = real_path(&spec.cwd);
    for arg in &mut spec.args {
        *arg = expand_svc_cwd(arg, &spec.cwd);
    }
    for root in &mut spec.sandbox.writable_roots {
        *root = real_path(root);
    }
}

#[must_use]
pub fn expand_svc_cwd(value: &str, cwd: &str) -> String {
    value.replace(SVC_CWD_PLACEHOLDER, cwd)
}

fn real_path(path: &str) -> String {
    std::fs::canonicalize(path).map_or_else(
        |_unresolved| path.to_string(),
        |resolved| resolved.to_string_lossy().into_owned(),
    )
}

#[cfg(test)]
#[path = "tests/workspace_tests.rs"]
mod tests;
