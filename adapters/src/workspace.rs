use usecases::AppSpec;

pub async fn materialise_workspace(spec: &mut AppSpec) {
    tokio::fs::create_dir_all(&spec.cwd).await.ok();
    spec.cwd = real_path(&spec.cwd);
    for root in &mut spec.sandbox.writable_roots {
        *root = real_path(root);
    }
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
