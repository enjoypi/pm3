use thiserror::Error;

use super::{depgraph::DependencyNode, restart::RestartPolicy};
use crate::sandbox::{PolicyError, SandboxPolicy, validate_policy};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppSpec {
    pub name: String,
    pub script: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub env: Vec<(String, String)>,
    pub autorestart: bool,
    pub min_uptime_ms: u64,
    pub max_restarts: u32,
    pub restart_delay_ms: u64,
    pub schedule: Option<String>,
    pub depends_on: Vec<String>,
    pub sandbox: SandboxPolicy,
}

#[derive(Debug, Eq, PartialEq, Error)]
pub enum SpecError {
    #[error("cannot accept blank app name")]
    EmptyName,

    #[error("cannot accept all-digit app name '{0}': a selector would read it as a process id")]
    NumericName(String),

    #[error(
        "cannot accept app name '{0}' starting with a dot: it would escape the service directory"
    )]
    DottedName(String),

    #[error(
        "cannot accept app name '{name}': '{character}' is not allowed, use letters, digits, '-', '_' or '.'"
    )]
    UnsafeName { name: String, character: char },

    #[error("cannot accept blank script for app '{0}'")]
    EmptyScript(String),

    #[error("cannot accept relative cwd '{cwd}' for app '{app}': must be an absolute path")]
    RelativeCwd { app: String, cwd: String },

    #[error("cannot accept app '{0}' depending on itself")]
    SelfDependency(String),

    #[error("cannot accept min_uptime_ms 0 for app '{0}': must be >= 1")]
    InvalidMinUptime(String),

    #[error("cannot accept blank environment variable name for app '{0}'")]
    EmptyEnvKey(String),

    #[error("cannot accept blank schedule for app '{0}'")]
    EmptySchedule(String),

    #[error("cannot accept sandbox policy for app '{app}': {source}")]
    Sandbox { app: String, source: PolicyError },
}

impl AppSpec {
    #[must_use]
    pub const fn restart_policy(&self) -> RestartPolicy {
        RestartPolicy {
            autorestart: self.autorestart,
            min_uptime_ms: self.min_uptime_ms,
            max_restarts: self.max_restarts,
            restart_delay_ms: self.restart_delay_ms,
        }
    }

    #[must_use]
    pub const fn is_scheduled_task(&self) -> bool {
        self.schedule.is_some() && !self.autorestart
    }

    #[must_use]
    pub const fn dependency_node(&self) -> DependencyNode<'_> {
        DependencyNode {
            name: self.name.as_str(),
            depends_on: self.depends_on.as_slice(),
        }
    }
}

pub fn validate_app_name(name: &str) -> Result<(), SpecError> {
    if name.trim().is_empty() {
        return Err(SpecError::EmptyName);
    }
    if name.parse::<u32>().is_ok() {
        return Err(SpecError::NumericName(name.to_string()));
    }
    if name.starts_with('.') {
        return Err(SpecError::DottedName(name.to_string()));
    }
    name.chars()
        .find(|letter| !is_name_letter(*letter))
        .map_or(Ok(()), |character| {
            Err(SpecError::UnsafeName {
                name: name.to_string(),
                character,
            })
        })
}

const fn is_name_letter(letter: char) -> bool {
    letter.is_ascii_alphanumeric() || matches!(letter, '-' | '_' | '.')
}

pub fn validate_spec(spec: &AppSpec) -> Result<(), SpecError> {
    validate_app_name(&spec.name)?;
    if spec.script.trim().is_empty() {
        return Err(SpecError::EmptyScript(spec.name.clone()));
    }
    if !spec.cwd.starts_with('/') {
        return Err(SpecError::RelativeCwd {
            app: spec.name.clone(),
            cwd: spec.cwd.clone(),
        });
    }
    if spec.depends_on.iter().any(|dep| dep == &spec.name) {
        return Err(SpecError::SelfDependency(spec.name.clone()));
    }
    if spec.min_uptime_ms < 1 {
        return Err(SpecError::InvalidMinUptime(spec.name.clone()));
    }
    if spec.env.iter().any(|(key, _value)| key.trim().is_empty()) {
        return Err(SpecError::EmptyEnvKey(spec.name.clone()));
    }
    if spec
        .schedule
        .as_ref()
        .is_some_and(|cron| cron.trim().is_empty())
    {
        return Err(SpecError::EmptySchedule(spec.name.clone()));
    }
    validate_policy(&spec.sandbox).map_err(|source| SpecError::Sandbox {
        app: spec.name.clone(),
        source,
    })
}

#[cfg(test)]
#[path = "../test_helpers/process_spec_test_helpers.rs"]
mod test_helpers;
#[cfg(test)]
#[path = "../tests/process_spec_tests.rs"]
mod tests;
