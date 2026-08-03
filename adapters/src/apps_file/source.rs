use std::path::{Path, PathBuf};

use usecases::{AppSpec, SpecError, SpecResolveError, SpecResolver, validate_app_name};

use super::{
    env_file::{env_file_of, load_env_file},
    file::{AppsFileError, SpecDefaults, load_service_file, resolve_checked},
};
use crate::config::Pm3Config;

pub const SERVICE_FILE_SUFFIX: &str = "yaml";

const HOME_VARIABLE: &str = "HOME";

#[derive(Clone, Debug)]
pub struct SpecSource {
    pub cfg_dir: PathBuf,
    pub config: Pm3Config,
    pub home_dir: String,
    pub host_home: Option<String>,
    pub logs_dir: String,
    pub tmp_dir: Option<String>,
}

impl SpecSource {
    pub fn defaults(&self) -> Result<SpecDefaults<'_>, AppsFileError> {
        SpecDefaults::from_config(
            &self.config,
            &self.home_dir,
            &self.logs_dir,
            self.tmp_dir.as_deref(),
        )
    }

    pub fn service(&self, name: &str) -> Result<PathBuf, SpecError> {
        service_file_of(&self.cfg_dir, name)
    }

    pub async fn resolve_service(&self, name: &str) -> Result<AppSpec, AppsFileError> {
        let path = self.service(name)?;
        let entry = load_service_file(&path.to_string_lossy()).await?;
        if entry.name != name {
            return Err(AppsFileError::MissingApp(name.to_string()));
        }
        let mut spec = resolve_checked(&self.defaults()?, &entry)?;
        spec.env = self.resolve_environment(name).await?;
        Ok(spec)
    }

    async fn resolve_environment(
        &self,
        name: &str,
    ) -> Result<Vec<(String, String)>, AppsFileError> {
        let path = env_file_of(&self.cfg_dir, name)?;
        let declared = load_env_file(&path, self.host_home.as_deref()).await?;
        log_environment(name, declared.len());
        Ok(with_host_home(self.host_home.as_deref(), declared))
    }
}

fn with_host_home(home: Option<&str>, declared: Vec<(String, String)>) -> Vec<(String, String)> {
    let Some(home) = home else {
        return declared;
    };
    if declared.iter().any(|(key, _)| key == HOME_VARIABLE) {
        return declared;
    }
    let mut merged = vec![(HOME_VARIABLE.to_string(), home.to_string())];
    merged.extend(declared);
    merged
}

fn log_environment(app: &str, entries: usize) {
    tracing::debug!(
        feature = "service",
        action = "load_env",
        app,
        entries,
        "pm3 read the environment values that belong to an app",
    );
}

impl SpecResolver for SpecSource {
    async fn prepare(&self, name: &str) -> Result<AppSpec, SpecResolveError> {
        let mut spec = self
            .resolve_service(name)
            .await
            .map_err(|error| resolve_failure(name, &error))?;
        crate::workspace::materialise_workspace(&mut spec).await;
        Ok(spec)
    }
}

fn resolve_failure(name: &str, error: &AppsFileError) -> SpecResolveError {
    let reason = error.to_string();
    let name = name.to_string();
    if matches!(
        error,
        AppsFileError::MissingApp(_) | AppsFileError::Io { .. }
    ) {
        return SpecResolveError::Missing { name, reason };
    }
    SpecResolveError::Unusable { name, reason }
}

pub fn service_file_of(cfg_dir: &Path, name: &str) -> Result<PathBuf, SpecError> {
    validate_app_name(name)?;
    Ok(cfg_dir.join(format!("{name}.{SERVICE_FILE_SUFFIX}")))
}

#[cfg(test)]
#[path = "../tests/apps_file_source_tests.rs"]
mod tests;
