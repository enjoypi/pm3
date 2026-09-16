use std::path::{Path, PathBuf};

use thiserror::Error;

pub(crate) const SOCKET_FILE: &str = "pm3.sock";
pub(crate) const PID_FILE: &str = "pm3.pid";
pub(crate) const LOCK_FILE: &str = "pm3.lock";
pub const CONFIG_FILE: &str = "config.yaml";
pub(crate) const DUMP_FILE: &str = "dump.yaml";
pub(crate) const DAEMON_LOG_FILE: &str = "pm3.log";
pub const LOGS_DIR: &str = "logs";
pub const APPS_DIR: &str = "apps";
pub const BACKUPS_DIR: &str = "install-backups";
pub const DEFAULT_HOME: &str = "~/.pm3";

const PM3_SUBDIR: &str = "pm3";
const RUNTIME_SUBDIR: &str = "run";
const XDG_CONFIG_FALLBACK: &str = "~/.config";
const XDG_STATE_FALLBACK: &str = "~/.local/state";
const XDG_DATA_FALLBACK: &str = "~/.local/share";
const RUNTIME_DIR_ROOT: &str = "/run/user";
const SUN_LEN_LIMIT: usize = 104;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pm3Roots {
    pub config: PathBuf,
    pub state: PathBuf,
    pub runtime: PathBuf,
    pub data: PathBuf,
}

impl Pm3Roots {
    #[must_use]
    pub fn single(root: &Path) -> Self {
        Self {
            config: root.to_path_buf(),
            state: root.to_path_buf(),
            runtime: root.to_path_buf(),
            data: root.to_path_buf(),
        }
    }

    #[must_use]
    pub const fn split(config: PathBuf, state: PathBuf, runtime: PathBuf, data: PathBuf) -> Self {
        Self {
            config,
            state,
            runtime,
            data,
        }
    }

    #[must_use]
    pub fn is_single(&self) -> bool {
        self.config == self.state && self.state == self.runtime && self.runtime == self.data
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pm3Paths {
    pub root: PathBuf,
    pub roots: Pm3Roots,
    pub socket: PathBuf,
    pub pid_file: PathBuf,
    pub lock_file: PathBuf,
    pub config_file: PathBuf,
    pub dump_file: PathBuf,
    pub logs_dir: PathBuf,
    pub daemon_log: PathBuf,
    pub apps_dir: PathBuf,
    pub backups_dir: PathBuf,
}

pub struct RuntimeSources<'s> {
    pub declared: Option<&'s str>,
    pub xdg: Option<&'s str>,
    pub uid: Option<u32>,
    pub state: &'s Path,
    pub exists: fn(&Path) -> bool,
}

#[derive(Debug, Eq, PartialEq, Error)]
pub enum PathError {
    #[error("cannot resolve pm3.home '{0}': no HOME in the environment to expand '~'")]
    MissingHome(String),

    #[error("cannot resolve pm3.home '{0}': must be absolute or start with '~'")]
    NotAbsolute(String),

    #[error(
        "cannot resolve pm3.home '{0}': expanding another user's home ('~name') is not supported"
    )]
    NamedHome(String),

    #[error(
        "cannot accept the socket path '{path}': {length} bytes exceeds the {limit} the operating system allows for a unix socket"
    )]
    SocketTooLong {
        path: String,
        length: usize,
        limit: usize,
    },
}

#[must_use]
pub fn resolve_paths(roots: Pm3Roots) -> Pm3Paths {
    let apps_dir = if roots.is_single() {
        roots.state.clone()
    } else {
        roots.state.join(APPS_DIR)
    };
    Pm3Paths {
        root: roots.state.clone(),
        socket: roots.runtime.join(SOCKET_FILE),
        pid_file: roots.runtime.join(PID_FILE),
        lock_file: roots.runtime.join(LOCK_FILE),
        config_file: roots.config.join(CONFIG_FILE),
        dump_file: roots.state.join(DUMP_FILE),
        logs_dir: roots.state.join(LOGS_DIR),
        daemon_log: roots.state.join(DAEMON_LOG_FILE),
        apps_dir,
        backups_dir: roots.data.join(BACKUPS_DIR),
        roots,
    }
}

pub fn resolve_config_root(
    declared: Option<&str>,
    xdg: Option<&str>,
    home_env: Option<&str>,
) -> Result<PathBuf, PathError> {
    resolve_xdg_root(declared, xdg, XDG_CONFIG_FALLBACK, home_env)
}

pub fn resolve_state_root(
    declared: Option<&str>,
    xdg: Option<&str>,
    home_env: Option<&str>,
) -> Result<PathBuf, PathError> {
    resolve_xdg_root(declared, xdg, XDG_STATE_FALLBACK, home_env)
}

pub fn resolve_data_root(
    declared: Option<&str>,
    xdg: Option<&str>,
    home_env: Option<&str>,
) -> Result<PathBuf, PathError> {
    resolve_xdg_root(declared, xdg, XDG_DATA_FALLBACK, home_env)
}

fn resolve_xdg_root(
    declared: Option<&str>,
    xdg: Option<&str>,
    fallback: &str,
    home_env: Option<&str>,
) -> Result<PathBuf, PathError> {
    if let Some(root) = named(declared) {
        return expand_home(root, home_env);
    }
    let base = named(xdg).unwrap_or(fallback);
    Ok(expand_home(base, home_env)?.join(PM3_SUBDIR))
}

#[must_use]
pub fn resolve_runtime_root(sources: &RuntimeSources<'_>) -> PathBuf {
    if let Some(root) = named(sources.declared) {
        return PathBuf::from(root);
    }
    if let Some(root) = named(sources.xdg) {
        return Path::new(root).join(PM3_SUBDIR);
    }
    if let Some(owned) = per_user_runtime_dir(sources.uid, sources.exists) {
        return owned;
    }
    sources.state.join(RUNTIME_SUBDIR)
}

fn per_user_runtime_dir(uid: Option<u32>, exists: fn(&Path) -> bool) -> Option<PathBuf> {
    let owner = uid?;
    let base = Path::new(RUNTIME_DIR_ROOT).join(owner.to_string());
    exists(&base).then(|| base.join(PM3_SUBDIR))
}

fn named(value: Option<&str>) -> Option<&str> {
    value.filter(|text| !text.is_empty())
}

pub fn check_socket_length(socket: &Path) -> Result<(), PathError> {
    let path = socket.to_string_lossy();
    let length = path.len();
    if length > SUN_LEN_LIMIT {
        return Err(PathError::SocketTooLong {
            path: path.into_owned(),
            length,
            limit: SUN_LEN_LIMIT,
        });
    }
    Ok(())
}
pub fn expand_home(raw: &str, home_env: Option<&str>) -> Result<PathBuf, PathError> {
    if let Some(suffix) = raw.strip_prefix('~') {
        if !suffix.is_empty() && !suffix.starts_with('/') {
            return Err(PathError::NamedHome(raw.to_string()));
        }
        let Some(home) = home_env.filter(|value| !value.is_empty()) else {
            return Err(PathError::MissingHome(raw.to_string()));
        };
        let trimmed = suffix.trim_start_matches('/');
        if trimmed.is_empty() {
            return Ok(PathBuf::from(home));
        }
        return Ok(Path::new(home).join(trimmed));
    }
    if Path::new(raw).is_absolute() {
        return Ok(PathBuf::from(raw));
    }
    Err(PathError::NotAbsolute(raw.to_string()))
}

pub fn default_config_path(
    pm3_home_env: Option<&str>,
    pm3_config_env: Option<&str>,
    xdg_config_env: Option<&str>,
    home_env: Option<&str>,
) -> Result<PathBuf, PathError> {
    if let Some(root) = named(pm3_home_env) {
        return Ok(expand_home(root, home_env)?.join(CONFIG_FILE));
    }
    Ok(resolve_config_root(pm3_config_env, xdg_config_env, home_env)?.join(CONFIG_FILE))
}

#[cfg(test)]
#[path = "tests/paths_tests.rs"]
mod tests;
