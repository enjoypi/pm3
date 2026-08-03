use std::{
    os::unix::fs::{MetadataExt as _, PermissionsExt as _},
    path::{Path, PathBuf},
};

use adapters::{Pm3Config, Pm3Paths, expand_home, pm3_variables, resolve_paths, runtime_dir_of};

use crate::{Error, Result};

const OWNER_ONLY_DIR: u32 = 0o700;
const RUNTIME_DIR_VARIABLE: &str = "XDG_RUNTIME_DIR";
const OWN_PROCESS_DIR: &str = "/proc/self";

pub fn resolve_layout(pm3: &Pm3Config, home_env: Option<&str>) -> Result<Pm3Paths> {
    let root = expand_home(&pm3.home, home_env)?;
    Ok(resolve_paths(&root))
}

pub fn resolve_cfg_dir(pm3: &Pm3Config, home_env: Option<&str>) -> Result<PathBuf> {
    Ok(expand_home(&pm3.cfg_dir, home_env)?)
}

pub fn canonicalize<F: FnOnce(String) -> Error>(path: &str, wrap: F) -> Result<PathBuf> {
    std::fs::canonicalize(path).map_err(|error| wrap(error.to_string()))
}

pub async fn ensure_layout(paths: &Pm3Paths, cfg_dir: &Path) -> Result<()> {
    prepare_home(&paths.root).await?;
    tokio::fs::create_dir_all(&paths.logs_dir)
        .await
        .map_err(|e| layout_error(&paths.logs_dir, &e))?;
    tokio::fs::create_dir_all(cfg_dir)
        .await
        .map_err(|e| layout_error(cfg_dir, &e))
}

async fn prepare_home(root: &Path) -> Result<()> {
    tokio::fs::create_dir_all(root)
        .await
        .map_err(|e| layout_error(root, &e))?;
    restrict_to_owner(root).await
}

async fn restrict_to_owner(path: &Path) -> Result<()> {
    let permissions = std::fs::Permissions::from_mode(OWNER_ONLY_DIR);
    tokio::fs::set_permissions(path, permissions)
        .await
        .map_err(|e| layout_error(path, &e))
}

pub async fn write_pid_file(paths: &Pm3Paths) -> Result<()> {
    let pid = std::process::id().to_string();
    tokio::fs::write(&paths.pid_file, pid)
        .await
        .map_err(|e| layout_error(&paths.pid_file, &e))
}

pub async fn read_pid_file(paths: &Pm3Paths) -> Option<u32> {
    let raw = tokio::fs::read_to_string(&paths.pid_file).await.ok()?;
    raw.trim().parse().ok()
}

pub async fn clear_runtime_files(paths: &Pm3Paths) {
    tokio::fs::remove_file(&paths.pid_file).await.ok();
    tokio::fs::remove_file(&paths.socket).await.ok();
}

#[must_use]
pub fn host_home() -> Option<String> {
    std::env::var("HOME").ok()
}

#[must_use]
pub fn host_pm3_home() -> Option<String> {
    std::env::var("PM3_HOME").ok()
}

#[must_use]
pub fn host_pm3_env() -> Vec<(String, String)> {
    pm3_variables(std::env::vars().collect())
}

#[must_use]
pub fn host_uid() -> Option<u32> {
    std::fs::metadata(OWN_PROCESS_DIR)
        .ok()
        .map(|owner| owner.uid())
}

#[must_use]
pub fn host_runtime_dir() -> Option<String> {
    let declared = std::env::var(RUNTIME_DIR_VARIABLE).ok();
    runtime_dir_of(declared.as_deref(), host_uid())
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
