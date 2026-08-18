use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use serde::Deserialize;
use thiserror::Error;
use usecases::{
    AppSpec, ReadScope, ReadyProbe, SandboxMode, SandboxPolicy, SpecError, parse_memory_limit,
    validate_forbidden_roots, validate_spec,
};

use super::roots::dedup_roots;
use crate::{
    config::{ConfigLoadError, Pm3Config, RestartConfig, substitute_env_vars},
    program::resolve_program,
    schedule::{CronError, validate_cron},
};

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
    #[serde(default, rename = "env")]
    pub rejected_env: Option<BTreeMap<String, String>>,
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
    pub max_restart_delay_ms: Option<u64>,
    #[serde(default)]
    pub listen_timeout_ms: Option<u64>,
    #[serde(default)]
    pub ready_probe: Option<ReadyProbeEntry>,
    #[serde(default)]
    pub schedule: Option<String>,
    #[serde(default)]
    pub max_memory: Option<String>,
    #[serde(default)]
    pub stop_exit_codes: Vec<i32>,
    #[serde(default)]
    pub sandbox: Option<SandboxEntry>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SandboxEntry {
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub read: Option<String>,
    #[serde(default)]
    pub network: Option<bool>,
    #[serde(default)]
    pub writable_roots: Option<Vec<String>>,
    #[serde(default)]
    pub readable_roots: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ReadyProbeEntry {
    #[serde(default)]
    pub exec: Option<Vec<String>>,
    #[serde(default)]
    pub tcp: Option<String>,
}

#[derive(Copy, Clone, Debug)]
pub struct SpecDefaults<'d> {
    pub restart: RestartConfig,
    pub sandbox_mode: SandboxMode,
    pub sandbox_read: ReadScope,
    pub sandbox_network: bool,
    pub forbidden_writable_roots: &'d [String],
    pub home_dir: &'d str,
    pub cfg_dir: &'d str,
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

    #[error(
        "cannot accept 'env' in the declaration for app '{app}': move the environment values to '{app}.env' beside the service file, so secrets never land in a yaml file"
    )]
    EnvInYaml { app: String },

    #[error("cannot find app '{0}' in its own service file")]
    MissingApp(String),

    #[error("cannot find '{program}' for app '{app}' on pm3.search_path")]
    ProgramNotFound { app: String, program: String },

    #[error(
        "cannot accept sandbox mode '{mode}' for {scope}: must be one of read-only, workspace-write, danger-full-access"
    )]
    InvalidSandboxMode { scope: String, mode: String },

    #[error("cannot accept sandbox read '{read}' for {scope}: must be one of full, minimal")]
    InvalidSandboxRead { scope: String, read: String },

    #[error(
        "cannot accept max_memory '{limit}' for app '{app}': use a byte count or a size like 300M"
    )]
    InvalidMemoryLimit { app: String, limit: String },

    #[error("cannot accept ready_probe for app '{app}': {reason}")]
    InvalidReadyProbe { app: String, reason: String },

    #[error(transparent)]
    EnvFile(#[from] super::env_file::EnvFileError),

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
        cfg_dir: &'d str,
        logs_dir: &'d str,
        tmp_dir: Option<&'d str>,
    ) -> Result<Self, AppsFileError> {
        let sandbox_mode = parse_mode(DEFAULTS_SCOPE, &pm3.sandbox.mode)?;
        let sandbox_read = parse_read(DEFAULTS_SCOPE, &pm3.sandbox.read)?;
        Ok(Self {
            restart: pm3.restart,
            sandbox_mode,
            sandbox_read,
            sandbox_network: pm3.sandbox.network,
            forbidden_writable_roots: &pm3.sandbox.forbidden_writable_roots,
            home_dir,
            cfg_dir,
            search_path: &pm3.search_path,
            logs_dir,
            tmp_dir,
        })
    }
}

pub async fn load_apps_file(path: &str) -> Result<AppsFile, AppsFileError> {
    parse_apps_file(&read_substituted(path).await?)
}

fn check_declared_names(apps: &AppsFile) -> Result<(), AppsFileError> {
    if apps.apps.is_empty() {
        return Err(AppsFileError::NoApps);
    }
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for entry in &apps.apps {
        if !seen.insert(entry.name.as_str()) {
            return Err(AppsFileError::DuplicateName(entry.name.clone()));
        }
        reject_env(entry)?;
    }
    Ok(())
}

fn reject_env(entry: &AppEntry) -> Result<(), AppsFileError> {
    if entry.rejected_env.is_some() {
        return Err(AppsFileError::EnvInYaml {
            app: entry.name.clone(),
        });
    }
    Ok(())
}

pub async fn load_service_file(path: &str) -> Result<AppEntry, AppsFileError> {
    parse_service_file(&read_substituted(path).await?)
}

async fn read_substituted(path: &str) -> Result<String, AppsFileError> {
    let raw = tokio::fs::read_to_string(path)
        .await
        .map_err(|source| AppsFileError::Io {
            path: path.to_string(),
            source,
        })?;
    Ok(substitute_env_vars(&raw)?)
}

pub fn parse_apps_file(yaml: &str) -> Result<AppsFile, AppsFileError> {
    let apps: AppsFile =
        serde_yaml2::from_str(yaml).map_err(|e| AppsFileError::Parse(e.to_string()))?;
    check_declared_names(&apps)?;
    Ok(apps)
}

pub fn parse_service_file(yaml: &str) -> Result<AppEntry, AppsFileError> {
    serde_yaml2::from_str(yaml).map_err(|e| AppsFileError::Parse(e.to_string()))
}

pub fn resolve_checked(
    defaults: &SpecDefaults<'_>,
    entry: &AppEntry,
) -> Result<AppSpec, AppsFileError> {
    let spec = resolve_entry(defaults, entry)?;
    validate_spec(&spec)?;
    validate_forbidden_roots(&spec.sandbox, defaults.forbidden_writable_roots).map_err(
        |source| SpecError::Sandbox {
            app: spec.name.clone(),
            source,
        },
    )?;
    if let Some(cron) = spec.schedule.as_deref() {
        validate_cron(&spec.name, cron)?;
    }
    Ok(spec)
}

fn resolve_entry(defaults: &SpecDefaults<'_>, entry: &AppEntry) -> Result<AppSpec, AppsFileError> {
    reject_env(entry)?;
    let cwd = working_directory(defaults, entry);
    let script = resolve_program(&entry.script, Some(defaults.search_path)).ok_or_else(|| {
        AppsFileError::ProgramNotFound {
            app: entry.name.clone(),
            program: entry.script.clone(),
        }
    })?;
    let sandbox = resolve_sandbox(defaults, entry, &cwd)?;
    let max_memory_kib = resolve_memory_limit(entry)?;
    let ready_probe = resolve_ready_probe(entry)?;
    Ok(AppSpec {
        max_memory_kib,
        ready_probe,
        listen_timeout_ms: entry.listen_timeout_ms,
        name: entry.name.clone(),
        script: script.to_string_lossy().into_owned(),
        args: entry.args.clone(),
        cwd,
        env: Vec::new(),
        autorestart: entry.autorestart.unwrap_or(defaults.restart.autorestart),
        min_uptime_ms: entry
            .min_uptime_ms
            .unwrap_or(defaults.restart.min_uptime_ms),
        max_restarts: entry.max_restarts.unwrap_or(defaults.restart.max_restarts),
        restart_delay_ms: entry
            .restart_delay_ms
            .unwrap_or(defaults.restart.restart_delay_ms),
        max_restart_delay_ms: entry
            .max_restart_delay_ms
            .unwrap_or(defaults.restart.max_restart_delay_ms),
        schedule: entry.schedule.clone(),
        depends_on: entry.depends_on.clone(),
        stop_exit_codes: entry.stop_exit_codes.clone(),
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
    let read = declared
        .and_then(|section| section.read.as_deref())
        .map(|raw| parse_read(&format!("app '{}'", entry.name), raw))
        .transpose()?
        .unwrap_or(defaults.sandbox_read);
    let network = declared
        .and_then(|section| section.network)
        .unwrap_or(defaults.sandbox_network);
    let writable_roots = declared
        .and_then(|section| section.writable_roots.clone())
        .unwrap_or_default();
    let readable_roots = declared
        .and_then(|section| section.readable_roots.clone())
        .unwrap_or_default();
    Ok(SandboxPolicy {
        mode,
        read,
        network,
        writable_roots,
        readable_roots,
        derived_readable_roots: Vec::new(),
        derived_roots: default_writable_roots(defaults, mode, cwd),
        unreadable_roots: pm3_owned_roots(defaults),
    })
}

fn resolve_memory_limit(entry: &AppEntry) -> Result<Option<u64>, AppsFileError> {
    let Some(declared) = entry.max_memory.as_deref() else {
        return Ok(None);
    };
    parse_memory_limit(declared)
        .map(Some)
        .ok_or_else(|| AppsFileError::InvalidMemoryLimit {
            app: entry.name.clone(),
            limit: declared.to_string(),
        })
}

fn resolve_ready_probe(entry: &AppEntry) -> Result<Option<ReadyProbe>, AppsFileError> {
    let Some(section) = &entry.ready_probe else {
        return Ok(None);
    };
    match (&section.exec, &section.tcp) {
        (Some(command), None) => Ok(Some(ReadyProbe::Exec {
            command: command.clone(),
        })),
        (None, Some(endpoint)) => parse_tcp_endpoint(&entry.name, endpoint).map(Some),
        (Some(_), Some(_)) => Err(invalid_ready_probe(
            &entry.name,
            "use exactly one of exec or tcp",
        )),
        (None, None) => Err(invalid_ready_probe(&entry.name, "declare exec or tcp")),
    }
}

fn parse_tcp_endpoint(app: &str, endpoint: &str) -> Result<ReadyProbe, AppsFileError> {
    let Some((host, port)) = endpoint.rsplit_once(':') else {
        return Err(invalid_ready_probe(app, "use host:port"));
    };
    let Ok(port) = port.parse::<u16>() else {
        return Err(invalid_ready_probe(
            app,
            "the port must be a number of 1-65535",
        ));
    };
    Ok(ReadyProbe::Tcp {
        host: host.to_string(),
        port,
    })
}

fn invalid_ready_probe(app: &str, reason: &str) -> AppsFileError {
    AppsFileError::InvalidReadyProbe {
        app: app.to_string(),
        reason: reason.to_string(),
    }
}

fn pm3_owned_roots(defaults: &SpecDefaults<'_>) -> Vec<String> {
    let candidates = [defaults.home_dir, defaults.cfg_dir]
        .into_iter()
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    dedup_roots(candidates)
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

fn parse_read(scope: &str, raw: &str) -> Result<ReadScope, AppsFileError> {
    ReadScope::parse(raw).ok_or_else(|| AppsFileError::InvalidSandboxRead {
        scope: scope.to_string(),
        read: raw.to_string(),
    })
}

#[cfg(test)]
#[path = "../test_helpers/apps_file_test_helpers.rs"]
mod test_helpers;
#[cfg(test)]
#[path = "../tests/apps_file_tests.rs"]
mod tests;
