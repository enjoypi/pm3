#[cfg(unix)]
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

use adapters::{
    Pm3Config, Pm3Paths, expand_home, pm3_variables, resolve_paths, runtime_dir_of, write_private,
};

use crate::{Error, Result};

#[cfg(unix)]
const OWNER_ONLY_DIR: u32 = 0o700;
const RUNTIME_DIR_VARIABLE: &str = "XDG_RUNTIME_DIR";
#[cfg(unix)]
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
    restrict_to_owner(&paths.logs_dir).await;
    tokio::fs::create_dir_all(cfg_dir)
        .await
        .map_err(|e| layout_error(cfg_dir, &e))?;
    restrict_to_owner(cfg_dir).await;
    Ok(())
}

async fn prepare_home(root: &Path) -> Result<()> {
    tokio::fs::create_dir_all(root)
        .await
        .map_err(|e| layout_error(root, &e))?;
    restrict_to_owner(root).await;
    Ok(())
}

#[cfg(unix)]
async fn restrict_to_owner(path: &Path) {
    let permissions = std::fs::Permissions::from_mode(OWNER_ONLY_DIR);
    if let Err(error) = tokio::fs::set_permissions(path, permissions).await {
        log_stuck_permissions(path, &error.to_string());
    }
}

#[cfg(not(unix))]
#[expect(
    clippy::unused_async,
    reason = "NTFS user directories are already per-user; the signature stays uniform"
)]
async fn restrict_to_owner(_path: &Path) {}

#[cfg(unix)]
fn log_stuck_permissions(path: &Path, reason: &str) {
    let path = path.to_string_lossy().into_owned();
    tracing::warn!(
        feature = "service",
        action = "restrict_directory",
        path,
        reason,
        "pm3 cannot keep a directory to its owner, so its contents stay readable by other users",
    );
}

#[cfg(windows)]
#[must_use]
pub fn pipe_name_of(socket: &Path) -> String {
    use std::hash::{Hash, Hasher as _};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    socket.hash(&mut hasher);
    format!(r"\\.\pipe\pm3-{:016x}", hasher.finish())
}

pub async fn write_pid_file(paths: &Pm3Paths) -> Result<()> {
    let pid = std::process::id().to_string();
    write_private(&paths.pid_file, &pid)
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
    home_of(std::env::var("HOME").ok())
}

#[cfg(windows)]
fn home_of(home: Option<String>) -> Option<String> {
    home.or_else(host_profile_home)
}

#[cfg(not(windows))]
const fn home_of(home: Option<String>) -> Option<String> {
    home
}

#[cfg(windows)]
fn host_profile_home() -> Option<String> {
    if let Ok(profile) = std::env::var("USERPROFILE") {
        return Some(profile);
    }
    let drive = std::env::var("HOMEDRIVE").ok()?;
    let path = std::env::var("HOMEPATH").ok()?;
    Some(format!("{drive}{path}"))
}

#[must_use]
pub fn host_pm3_home() -> Option<String> {
    std::env::var("PM3_HOME").ok()
}

#[must_use]
pub fn host_install_destination() -> Option<String> {
    std::env::var("PM3_INSTALL_PATH").ok()
}

#[must_use]
pub fn host_install_backups() -> Option<String> {
    std::env::var("PM3_INSTALL_BACKUPS").ok()
}

#[must_use]
pub fn host_pm3_env() -> Vec<(String, String)> {
    pm3_variables(std::env::vars().collect())
}

#[cfg(unix)]
#[must_use]
pub fn host_uid() -> Option<u32> {
    owner_uid_of(Path::new(OWN_PROCESS_DIR))
}

#[cfg(not(unix))]
#[must_use]
pub const fn host_uid() -> Option<u32> {
    None
}

#[cfg(unix)]
#[must_use]
pub fn owner_uid_of(path: &Path) -> Option<u32> {
    std::fs::metadata(path).ok().map(|owner| owner.uid())
}

#[cfg(not(unix))]
#[must_use]
pub const fn owner_uid_of(_path: &Path) -> Option<u32> {
    None
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
