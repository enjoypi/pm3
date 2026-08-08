use std::path::{Path, PathBuf};

pub const DAEMON_SUBCOMMAND: &str = "daemon";
pub const CONFIG_FLAG: &str = "--config";

const LAUNCHD_UNIT_DIR: &str = "Library/LaunchAgents";
const LAUNCHD_UNIT_SUFFIX: &str = "plist";
const SYSTEMD_UNIT_DIR: &str = ".config/systemd/user";
const SYSTEMD_UNIT_SUFFIX: &str = "service";

const LAUNCHD_PID_KEY: &str = "\"PID\"";
const SYSTEMD_ACTIVE: &str = "active";
const PM3_VARIABLE_PREFIX: &str = "PM3_";
const LINGER_ENABLED: &str = "yes";
const RUNTIME_DIR_ROOT: &str = "/run/user";

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum UnitKind {
    Launchd,
    Systemd,
}

impl UnitKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Launchd => "launchd",
            Self::Systemd => "systemd",
        }
    }

    #[must_use]
    pub const fn unit_dir(self) -> &'static str {
        match self {
            Self::Launchd => LAUNCHD_UNIT_DIR,
            Self::Systemd => SYSTEMD_UNIT_DIR,
        }
    }

    #[must_use]
    pub const fn unit_suffix(self) -> &'static str {
        match self {
            Self::Launchd => LAUNCHD_UNIT_SUFFIX,
            Self::Systemd => SYSTEMD_UNIT_SUFFIX,
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum LingerState {
    Enabled,
    Unknown,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum UnitStatus {
    NotInstalled,
    InstalledNotRunning,
    Running,
}

impl UnitStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotInstalled => "not installed",
            Self::InstalledNotRunning => "installed, not running",
            Self::Running => "running",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnitSpec {
    pub kind: UnitKind,
    pub label: String,
    pub unit_dir: PathBuf,
    pub program: PathBuf,
    pub config_path: PathBuf,
    pub working_directory: PathBuf,
    pub log_path: PathBuf,
    pub search_path: String,
    pub home: String,
    pub pm3_env: Vec<(String, String)>,
    pub restart_delay_secs: u64,
    pub restart_condition: String,
    pub umask: u32,
    pub max_tasks: u64,
    pub cpu_quota_percent: u64,
}

impl UnitSpec {
    #[must_use]
    pub fn unit_name(&self) -> String {
        format!("{}.{}", self.label, self.kind.unit_suffix())
    }

    #[must_use]
    pub fn unit_path(&self) -> PathBuf {
        self.unit_dir.join(self.unit_name())
    }

    #[must_use]
    pub fn daemon_args(&self) -> [String; 3] {
        [
            DAEMON_SUBCOMMAND.to_string(),
            CONFIG_FLAG.to_string(),
            self.config_path.to_string_lossy().into_owned(),
        ]
    }
}

#[must_use]
pub fn unit_dir_of(kind: UnitKind, home: &Path) -> PathBuf {
    home.join(kind.unit_dir())
}

#[must_use]
pub fn pm3_variables(vars: Vec<(String, String)>) -> Vec<(String, String)> {
    let mut kept: Vec<(String, String)> = vars
        .into_iter()
        .filter(|(name, _value)| name.starts_with(PM3_VARIABLE_PREFIX))
        .collect();
    kept.sort();
    kept
}

#[must_use]
pub fn runtime_dir_of(declared: Option<&str>, uid: Option<u32>) -> Option<String> {
    match declared {
        Some(dir) if !dir.is_empty() => Some(dir.to_string()),
        _ => uid.map(|owner| format!("{RUNTIME_DIR_ROOT}/{owner}")),
    }
}

#[must_use]
pub fn parse_linger_state(exit_success: bool, stdout: &str) -> LingerState {
    if exit_success && stdout.trim() == LINGER_ENABLED {
        return LingerState::Enabled;
    }
    LingerState::Unknown
}

#[must_use]
pub fn parse_run_state(kind: UnitKind, exit_success: bool, stdout: &str) -> bool {
    match kind {
        UnitKind::Launchd => exit_success && stdout.contains(LAUNCHD_PID_KEY),
        UnitKind::Systemd => stdout.trim() == SYSTEMD_ACTIVE,
    }
}

#[must_use]
pub fn parse_launchd_pid(stdout: &str) -> Option<u32> {
    for line in stdout.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() != LAUNCHD_PID_KEY {
            continue;
        }
        return value.trim().trim_end_matches(';').trim().parse().ok();
    }
    None
}

#[must_use]
pub fn parse_main_pid(stdout: &str) -> Option<u32> {
    stdout.trim().parse::<u32>().ok().filter(|pid| *pid > 0)
}

#[cfg(test)]
#[path = "../tests/unit_spec_tests.rs"]
mod tests;
