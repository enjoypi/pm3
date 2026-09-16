#[cfg(unix)]
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::{
    path::{Path, PathBuf},
    sync::LazyLock,
};

use adapters::{
    Pm3Config, Pm3Paths, Pm3Roots, RuntimeSources, check_socket_length, expand_home, pm3_variables,
    resolve_config_root, resolve_data_root, resolve_paths, resolve_runtime_root,
    resolve_state_root, runtime_dir_of, write_private,
};

use crate::{Error, Result};

#[cfg(unix)]
const OWNER_ONLY_DIR: u32 = 0o700;
const RUNTIME_DIR_VARIABLE: &str = "XDG_RUNTIME_DIR";
#[cfg(unix)]
const OWN_PROCESS_DIR: &str = "/proc/self";

pub struct RootSources<'s> {
    pub pm3_home: Option<&'s str>,
    pub config: Option<&'s str>,
    pub state: Option<&'s str>,
    pub runtime: Option<&'s str>,
    pub data: Option<&'s str>,
    pub xdg_config: Option<&'s str>,
    pub xdg_state: Option<&'s str>,
    pub xdg_runtime: Option<&'s str>,
    pub xdg_data: Option<&'s str>,
    pub home: Option<&'s str>,
    pub uid: Option<u32>,
    pub exists: fn(&Path) -> bool,
}

fn roots_from(pm3: &Pm3Config, sources: &RootSources<'_>) -> Result<Pm3Roots> {
    if let Some(root) = overriding(sources.pm3_home, &pm3.home) {
        return Ok(Pm3Roots::single(&expand_home(root, sources.home)?));
    }
    let state = resolve_state_root(
        overriding(sources.state, &pm3.state_dir),
        sources.xdg_state,
        sources.home,
    )?;
    let runtime = resolve_runtime_root(&RuntimeSources {
        declared: overriding(sources.runtime, &pm3.runtime_dir),
        xdg: sources.xdg_runtime,
        uid: sources.uid,
        state: &state,
        exists: sources.exists,
    });
    Ok(Pm3Roots::split(
        resolve_config_root(sources.config, sources.xdg_config, sources.home)?,
        state,
        runtime,
        resolve_data_root(
            overriding(sources.data, &pm3.data_dir),
            sources.xdg_data,
            sources.home,
        )?,
    ))
}

fn resolve_roots(pm3: &Pm3Config, home_env: Option<&str>) -> Result<Pm3Roots> {
    let declared_home = host_pm3_home();
    let declared_config = host_pm3_config_dir();
    let declared_state = host_pm3_state_dir();
    let declared_runtime = host_pm3_runtime_dir();
    let declared_data = host_pm3_data_dir();
    roots_from(
        pm3,
        &RootSources {
            pm3_home: declared_home.as_deref(),
            config: declared_config.as_deref(),
            state: declared_state.as_deref(),
            runtime: declared_runtime.as_deref(),
            data: declared_data.as_deref(),
            xdg_config: xdg_config_env(),
            xdg_state: xdg_state_env(),
            xdg_runtime: xdg_runtime_env(),
            xdg_data: xdg_data_env(),
            home: home_env,
            uid: host_uid(home_env),
            exists: directory_exists,
        },
    )
}

fn overriding<'v>(declared: Option<&'v str>, configured: &'v str) -> Option<&'v str> {
    if let Some(value) = named(declared.unwrap_or_default()) {
        return Some(value);
    }
    named(configured)
}

fn directory_exists(path: &Path) -> bool {
    path.is_dir()
}

const fn named(value: &str) -> Option<&str> {
    if value.is_empty() {
        return None;
    }
    Some(value)
}

pub fn resolve_cfg_dir(pm3: &Pm3Config, home_env: Option<&str>) -> Result<PathBuf> {
    Ok(resolve_places(pm3, home_env)?.cfg_dir)
}

#[derive(Debug)]
pub struct Pm3Places {
    pub paths: Pm3Paths,
    pub cfg_dir: PathBuf,
}

pub fn resolve_places(pm3: &Pm3Config, home_env: Option<&str>) -> Result<Pm3Places> {
    let roots = resolve_roots(pm3, home_env)?;
    let cfg_dir = match named(&pm3.cfg_dir) {
        Some(declared) => expand_home(declared, home_env)?,
        None => roots.config.clone(),
    };
    let paths = resolve_paths(roots);
    check_socket_length(&paths.socket)?;
    Ok(Pm3Places { paths, cfg_dir })
}

pub fn canonicalize<F: FnOnce(String) -> Error>(path: &str, wrap: F) -> Result<PathBuf> {
    std::fs::canonicalize(path).map_err(|error| wrap(error.to_string()))
}

pub async fn ensure_layout(paths: &Pm3Paths, cfg_dir: &Path) -> Result<()> {
    for root in owned_directories(paths, cfg_dir) {
        prepare_directory(root).await?;
    }
    Ok(())
}

fn owned_directories<'p>(paths: &'p Pm3Paths, cfg_dir: &'p Path) -> Vec<&'p Path> {
    let mut wanted = vec![
        paths.roots.state.as_path(),
        paths.roots.config.as_path(),
        paths.roots.runtime.as_path(),
        paths.roots.data.as_path(),
        paths.logs_dir.as_path(),
        paths.apps_dir.as_path(),
        cfg_dir,
    ];
    wanted.dedup();
    wanted
}

async fn prepare_directory(path: &Path) -> Result<()> {
    tokio::fs::create_dir_all(path)
        .await
        .map_err(|e| layout_error(path, &e))?;
    restrict_to_owner(path).await;
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
pub fn xdg_config_env() -> Option<&'static str> {
    xdg_value(&XDG_CONFIG)
}

#[must_use]
pub fn xdg_state_env() -> Option<&'static str> {
    xdg_value(&XDG_STATE)
}

#[must_use]
pub fn xdg_data_env() -> Option<&'static str> {
    xdg_value(&XDG_DATA)
}

#[must_use]
pub fn xdg_runtime_env() -> Option<&'static str> {
    xdg_value(&XDG_RUNTIME)
}

fn xdg_value(cell: &'static LazyLock<Option<String>>) -> Option<&'static str> {
    cell.as_deref().filter(|text| !text.is_empty())
}

static XDG_CONFIG: LazyLock<Option<String>> =
    LazyLock::new(|| std::env::var("XDG_CONFIG_HOME").ok());
static XDG_STATE: LazyLock<Option<String>> = LazyLock::new(|| std::env::var("XDG_STATE_HOME").ok());
static XDG_DATA: LazyLock<Option<String>> = LazyLock::new(|| std::env::var("XDG_DATA_HOME").ok());
static XDG_RUNTIME: LazyLock<Option<String>> =
    LazyLock::new(|| std::env::var(RUNTIME_DIR_VARIABLE).ok());

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
pub fn host_pm3_state_dir() -> Option<String> {
    std::env::var("PM3_STATE_DIR").ok()
}

#[must_use]
pub fn host_pm3_runtime_dir() -> Option<String> {
    std::env::var("PM3_RUNTIME_DIR").ok()
}

#[must_use]
pub fn host_pm3_data_dir() -> Option<String> {
    std::env::var("PM3_DATA_DIR").ok()
}

#[must_use]
pub fn host_pm3_config_dir() -> Option<String> {
    std::env::var("PM3_CONFIG_DIR").ok()
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
pub fn host_uid(home_env: Option<&str>) -> Option<u32> {
    let home = home_env.map(Path::new).and_then(owner_uid_of);
    owner_uid_of(Path::new(OWN_PROCESS_DIR)).or(home)
}

#[cfg(not(unix))]
#[must_use]
pub const fn host_uid(_home_env: Option<&str>) -> Option<u32> {
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
pub fn host_runtime_dir(home_env: Option<&str>) -> Option<String> {
    let declared = std::env::var(RUNTIME_DIR_VARIABLE).ok();
    runtime_dir_of(declared.as_deref(), host_uid(home_env))
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
