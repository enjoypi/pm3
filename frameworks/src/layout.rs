use std::path::Path;

use adapters::{Pm3Config, Pm3Paths, expand_home, resolve_paths};

use crate::{Error, Result};

pub fn resolve_layout(pm3: &Pm3Config, home_env: Option<&str>) -> Result<Pm3Paths> {
    let root = expand_home(&pm3.home, home_env)?;
    Ok(resolve_paths(&root))
}

pub async fn ensure_layout(paths: &Pm3Paths) -> Result<()> {
    tokio::fs::create_dir_all(&paths.logs_dir)
        .await
        .map_err(|e| layout_error(&paths.logs_dir, &e))
}

pub async fn write_pid_file(paths: &Pm3Paths) -> Result<()> {
    let pid = std::process::id().to_string();
    tokio::fs::write(&paths.pid_file, pid)
        .await
        .map_err(|e| layout_error(&paths.pid_file, &e))
}

pub async fn clear_runtime_files(paths: &Pm3Paths) {
    tokio::fs::remove_file(&paths.socket).await.ok();
    tokio::fs::remove_file(&paths.pid_file).await.ok();
}

#[must_use]
pub fn host_home() -> Option<String> {
    std::env::var("HOME").ok()
}

fn layout_error(path: &Path, source: &std::io::Error) -> Error {
    Error::Layout {
        path: path.to_string_lossy().into_owned(),
        reason: source.to_string(),
    }
}

#[cfg(test)]
#[path = "tests/layout_tests.rs"]
mod tests;
