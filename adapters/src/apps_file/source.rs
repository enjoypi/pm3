use std::path::{Path, PathBuf};

use usecases::{AppSpec, SpecError, validate_app_name};

use super::file::{AppsFileError, SpecDefaults, load_service_file, resolve_checked};
use crate::config::Pm3Config;

pub const SERVICE_FILE_SUFFIX: &str = "yaml";

#[derive(Clone, Debug)]
pub struct SpecSource {
    pub cfg_dir: PathBuf,
    pub config: Pm3Config,
    pub home_dir: String,
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

    pub fn service_file(&self, name: &str) -> Result<PathBuf, SpecError> {
        service_file_of(&self.cfg_dir, name)
    }

    pub async fn resolve_service(&self, name: &str) -> Result<AppSpec, AppsFileError> {
        let path = self.service_file(name)?;
        let entry = load_service_file(&path.to_string_lossy()).await?;
        if entry.name != name {
            return Err(AppsFileError::MissingApp(name.to_string()));
        }
        resolve_checked(&self.defaults()?, &entry)
    }
}

pub fn service_file_of(cfg_dir: &Path, name: &str) -> Result<PathBuf, SpecError> {
    validate_app_name(name)?;
    Ok(cfg_dir.join(format!("{name}.{SERVICE_FILE_SUFFIX}")))
}

#[cfg(test)]
#[path = "../tests/apps_file_source_tests.rs"]
mod tests;
