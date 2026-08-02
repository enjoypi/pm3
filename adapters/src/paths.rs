use std::path::{Path, PathBuf};

use thiserror::Error;

pub const SOCKET_FILE: &str = "pm3.sock";
pub const PID_FILE: &str = "pm3.pid";
pub const LOCK_FILE: &str = "pm3.lock";
pub const CONFIG_FILE: &str = "config.yaml";
pub const DUMP_FILE: &str = "dump.yaml";
pub const DAEMON_LOG_FILE: &str = "pm3.log";
pub const LOGS_DIR: &str = "logs";
pub const DEFAULT_HOME: &str = "~/.pm3";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pm3Paths {
    pub root: PathBuf,
    pub socket: PathBuf,
    pub pid_file: PathBuf,
    pub lock_file: PathBuf,
    pub config_file: PathBuf,
    pub dump_file: PathBuf,
    pub logs_dir: PathBuf,
    pub daemon_log: PathBuf,
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
}

#[must_use]
pub fn resolve_paths(root: &Path) -> Pm3Paths {
    Pm3Paths {
        root: root.to_path_buf(),
        socket: root.join(SOCKET_FILE),
        pid_file: root.join(PID_FILE),
        lock_file: root.join(LOCK_FILE),
        config_file: root.join(CONFIG_FILE),
        dump_file: root.join(DUMP_FILE),
        logs_dir: root.join(LOGS_DIR),
        daemon_log: root.join(DAEMON_LOG_FILE),
    }
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
    if raw.starts_with('/') {
        return Ok(PathBuf::from(raw));
    }
    Err(PathError::NotAbsolute(raw.to_string()))
}

pub fn default_config_path(
    pm3_home_env: Option<&str>,
    home_env: Option<&str>,
) -> Result<PathBuf, PathError> {
    let root = pm3_home_env
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_HOME);
    Ok(expand_home(root, home_env)?.join(CONFIG_FILE))
}

#[cfg(test)]
#[path = "tests/paths_tests.rs"]
mod tests;
