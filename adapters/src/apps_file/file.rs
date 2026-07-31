use std::{collections::BTreeMap, fs, path::Path};

use serde::Deserialize;
use thiserror::Error;
use usecases::{AppSpec, SandboxMode, SandboxPolicy, SpecError, validate_spec};

use super::roots::dedup_roots;
use crate::{
    config::{ConfigLoadError, Pm3Config, RestartConfig, substitute_env_vars},
    program::resolve_program,
    schedule::{CronError, validate_cron},
};

pub const DEFAULT_AUTORESTART: bool = true;
const DEFAULTS_SCOPE: &str = "pm3.sandbox";

#[derive(Clone, Debug, Deserialize)]
pub struct AppsFile {
    pub apps: Vec<AppEntry>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AppEntry {
    pub name: String,
    pub script: String,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub autorestart: Option<bool>,
    #[serde(default)]
    pub min_uptime_ms: Option<u64>,
    #[serde(default)]
    pub max_restarts: Option<u32>,
    #[serde(default)]
    pub restart_delay_ms: Option<u64>,
    #[serde(default)]
    pub schedule: Option<String>,
    #[serde(default)]
    pub sandbox: Option<SandboxEntry>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SandboxEntry {
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub network: Option<bool>,
    #[serde(default)]
    pub writable_roots: Option<Vec<String>>,
}

#[derive(Copy, Clone, Debug)]
pub struct SpecDefaults<'d> {
    pub restart: RestartConfig,
    pub sandbox_mode: SandboxMode,
    pub sandbox_network: bool,
    pub home_dir: &'d str,
    pub search_path: &'d str,
    pub logs_dir: &'d str,
    pub tmp_dir: Option<&'d str>,
}

#[derive(Debug, Error)]
pub enum AppsFileError {
    #[error("cannot read apps file '{path}': {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },

    #[error("cannot parse apps file: {0}")]
    Parse(String),

    #[error("cannot accept an apps file with no apps")]
    NoApps,

    #[error("cannot accept duplicate app name '{0}'")]
    DuplicateName(String),

    #[error("cannot accept environment entry '{0}': expected KEY=VALUE")]
    InvalidEnvPair(String),

    #[error("cannot find app '{0}' in its own service file")]
    MissingApp(String),

    #[error("cannot find '{program}' for app '{app}' on pm3.search_path")]
    ProgramNotFound { app: String, program: String },

    #[error(
        "cannot accept sandbox mode '{mode}' for {scope}: must be one of read-only, workspace-write, danger-full-access"
    )]
    InvalidSandboxMode { scope: String, mode: String },

    #[error(transparent)]
    Substitute(#[from] ConfigLoadError),

    #[error(transparent)]
    Spec(#[from] SpecError),

    #[error(transparent)]
    Cron(#[from] CronError),
}

impl<'d> SpecDefaults<'d> {
    pub fn from_config(
        pm3: &'d Pm3Config,
        home_dir: &'d str,
        logs_dir: &'d str,
        tmp_dir: Option<&'d str>,
    ) -> Result<Self, AppsFileError> {
        let sandbox_mode = parse_mode(DEFAULTS_SCOPE, &pm3.sandbox.mode)?;
        Ok(Self {
            restart: pm3.restart,
            sandbox_mode,
            sandbox_network: pm3.sandbox.network,
            home_dir,
            search_path: &pm3.search_path,
            logs_dir,
            tmp_dir,
        })
    }
}

pub fn load_apps_file(path: &str) -> Result<AppsFile, AppsFileError> {
    parse_apps_file(&read_substituted(path)?)
}

pub fn load_service_file(path: &str) -> Result<AppEntry, AppsFileError> {
    parse_service_file(&read_substituted(path)?)
}

fn read_substituted(path: &str) -> Result<String, AppsFileError> {
    let raw = fs::read_to_string(path).map_err(|source| AppsFileError::Io {
        path: path.to_string(),
        source,
    })?;
    Ok(substitute_env_vars(&raw)?)
}

pub fn parse_apps_file(yaml: &str) -> Result<AppsFile, AppsFileError> {
    serde_yaml2::from_str(yaml).map_err(|e| AppsFileError::Parse(e.to_string()))
}

pub fn parse_service_file(yaml: &str) -> Result<AppEntry, AppsFileError> {
    serde_yaml2::from_str(yaml).map_err(|e| AppsFileError::Parse(e.to_string()))
}

pub fn resolve_specs(
    defaults: &SpecDefaults<'_>,
    apps: &AppsFile,
) -> Result<Vec<AppSpec>, AppsFileError> {
    if apps.apps.is_empty() {
        return Err(AppsFileError::NoApps);
    }
    let mut specs: Vec<AppSpec> = Vec::with_capacity(apps.apps.len());
    for entry in &apps.apps {
        let spec = resolve_checked(defaults, entry)?;
        if specs.iter().any(|known| known.name == spec.name) {
            return Err(AppsFileError::DuplicateName(spec.name));
        }
        specs.push(spec);
    }
    Ok(specs)
}

pub fn resolve_checked(
    defaults: &SpecDefaults<'_>,
    entry: &AppEntry,
) -> Result<AppSpec, AppsFileError> {
    let spec = resolve_entry(defaults, entry)?;
    validate_spec(&spec)?;
    if let Some(cron) = spec.schedule.as_deref() {
        validate_cron(&spec.name, cron)?;
    }
    Ok(spec)
}

fn resolve_entry(defaults: &SpecDefaults<'_>, entry: &AppEntry) -> Result<AppSpec, AppsFileError> {
    let cwd = working_directory(defaults, entry);
    let script = resolve_program(&entry.script, Some(defaults.search_path)).ok_or_else(|| {
        AppsFileError::ProgramNotFound {
            app: entry.name.clone(),
            program: entry.script.clone(),
        }
    })?;
    let sandbox = resolve_sandbox(defaults, entry, &cwd)?;
    let env = entry
        .env
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    Ok(AppSpec {
        name: entry.name.clone(),
        script: script.to_string_lossy().into_owned(),
        args: entry.args.clone(),
        cwd,
        env,
        autorestart: entry.autorestart.unwrap_or(DEFAULT_AUTORESTART),
        min_uptime_ms: entry
            .min_uptime_ms
            .unwrap_or(defaults.restart.min_uptime_ms),
        max_restarts: entry.max_restarts.unwrap_or(defaults.restart.max_restarts),
        restart_delay_ms: entry
            .restart_delay_ms
            .unwrap_or(defaults.restart.restart_delay_ms),
        schedule: entry.schedule.clone(),
        depends_on: entry.depends_on.clone(),
        sandbox,
    })
}

fn working_directory(defaults: &SpecDefaults<'_>, entry: &AppEntry) -> String {
    entry.cwd.clone().unwrap_or_else(|| {
        Path::new(defaults.home_dir)
            .join(&entry.name)
            .to_string_lossy()
            .into_owned()
    })
}

fn resolve_sandbox(
    defaults: &SpecDefaults<'_>,
    entry: &AppEntry,
    cwd: &str,
) -> Result<SandboxPolicy, AppsFileError> {
    let declared = entry.sandbox.as_ref();
    let mode = declared
        .and_then(|section| section.mode.as_deref())
        .map(|raw| parse_mode(&format!("app '{}'", entry.name), raw))
        .transpose()?
        .unwrap_or(defaults.sandbox_mode);
    let network = declared
        .and_then(|section| section.network)
        .unwrap_or(defaults.sandbox_network);
    let (writable_roots, derived_roots) = declared
        .and_then(|section| section.writable_roots.clone())
        .map_or_else(
            || (Vec::new(), default_writable_roots(defaults, mode, cwd)),
            |roots| (roots, Vec::new()),
        );
    Ok(SandboxPolicy {
        mode,
        network,
        writable_roots,
        derived_roots,
    })
}

fn default_writable_roots(
    defaults: &SpecDefaults<'_>,
    mode: SandboxMode,
    cwd: &str,
) -> Vec<String> {
    if mode != SandboxMode::WorkspaceWrite {
        return Vec::new();
    }
    let candidates = [Some(cwd), Some(defaults.logs_dir), defaults.tmp_dir]
        .into_iter()
        .flatten()
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    dedup_roots(candidates)
}

fn parse_mode(scope: &str, raw: &str) -> Result<SandboxMode, AppsFileError> {
    SandboxMode::parse(raw).ok_or_else(|| AppsFileError::InvalidSandboxMode {
        scope: scope.to_string(),
        mode: raw.to_string(),
    })
}

#[cfg(test)]
#[path = "../test_helpers/apps_file_test_helpers.rs"]
mod test_helpers;
#[cfg(test)]
#[path = "../tests/apps_file_tests.rs"]
mod tests;
