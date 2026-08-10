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
pub fn pipe_name_of(socket: &Path, secret: &str) -> String {
    use std::hash::{Hash, Hasher as _};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    socket.hash(&mut hasher);
    secret.hash(&mut hasher);
    format!(r"\\.\pipe\pm3-{:016x}", hasher.finish())
}

#[cfg(windows)]
pub const PIPE_SECRET_FILE: &str = "pipe.secret";

#[cfg(windows)]
const PIPE_SECRET_LEN: usize = 32;

#[cfg(windows)]
#[must_use]
pub fn is_pipe_secret(secret: &str) -> bool {
    secret.len() == PIPE_SECRET_LEN && secret.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(windows)]
#[must_use]
pub fn generate_pipe_secret() -> String {
    let mut rng = fastrand::Rng::new();
    format!("{:016x}{:016x}", rng.u64(..), rng.u64(..))
}

#[cfg(windows)]
pub async fn pipe_secret(socket: &Path) -> Result<String> {
    let path = socket.with_file_name(PIPE_SECRET_FILE);
    match read_pipe_secret(&path).await? {
        Some(secret) => Ok(secret),
        None => create_pipe_secret(&path).await,
    }
}

#[cfg(windows)]
async fn read_pipe_secret(path: &Path) -> Result<Option<String>> {
    match tokio::fs::read_to_string(path).await {
        Ok(raw) => {
            let secret = raw.trim();
            if is_pipe_secret(secret) {
                return Ok(Some(secret.to_string()));
            }
            tokio::fs::remove_file(path)
                .await
                .map_err(|error| layout_error(path, &error))?;
            Ok(None)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(layout_error(path, &error)),
    }
}

#[cfg(windows)]
async fn create_pipe_secret(path: &Path) -> Result<String> {
    let secret = generate_pipe_secret();
    match create_secret_file(path, &secret).await {
        Ok(()) => return Ok(secret),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(layout_error(path, &error)),
    }
    read_pipe_secret(path)
        .await
        .map(|winner| winner.unwrap_or(secret))
}

#[cfg(windows)]
async fn create_secret_file(path: &Path, secret: &str) -> std::io::Result<()> {
    use tokio::io::AsyncWriteExt as _;
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .await?;
    file.write_all(secret.as_bytes()).await?;
    file.flush().await
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
    remove_runtime_file(&paths.pid_file).await;
    remove_runtime_file(&paths.socket).await;
}

pub(crate) async fn remove_runtime_file(path: &Path) {
    let Err(error) = tokio::fs::remove_file(path).await else {
        return;
    };
    if error.kind() == std::io::ErrorKind::NotFound {
        return;
    }
    log_stuck_removal(path, &error.to_string());
}

fn log_stuck_removal(path: &Path, reason: &str) {
    let path = path.to_string_lossy().into_owned();
    tracing::warn!(
        feature = "lifecycle",
        action = "remove_runtime_file",
        path,
        reason,
        "pm3 could not remove a runtime file, so shutdown watchers may misread the daemon state",
    );
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
