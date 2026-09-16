use std::path::{Path, PathBuf};

use usecases::{
    AppSpec, EnvOrigin, EnvScope, EnvValue, SpecError, SpecResolveError, SpecResolver,
    merge_environment, validate_app_name,
};

use super::{
    enc_file::{ENC_FILE_SUFFIX, enc_file_present, load_enc_file},
    env_file::{ENV_FILE_SUFFIX, load_env_file, parse_env_text},
    file::{AppsFileError, SpecDefaults, SpecRoots, load_service_file, resolve_checked},
    global_env::scoped,
};
use crate::config::Pm3Config;

pub const SERVICE_FILE_SUFFIX: &str = "yaml";

const HOME_VARIABLE: &str = "HOME";

#[derive(Clone, Debug)]
pub struct SpecSource {
    pub cfg_dir: PathBuf,
    pub config: Pm3Config,
    pub home_dir: String,
    pub apps_dir: String,
    pub state_dir: String,
    pub runtime_dir: String,
    pub data_dir: String,
    pub host_home: Option<String>,
    pub logs_dir: String,
    pub tmp_dir: Option<String>,
    pub global_env: Vec<EnvValue>,
}

impl SpecSource {
    pub fn defaults(&self) -> Result<SpecDefaults<'_>, AppsFileError> {
        SpecDefaults::from_config(
            &self.config,
            SpecRoots {
                home_dir: &self.home_dir,
                cfg_dir: self.cfg_dir.to_str().unwrap_or_default(),
                apps_dir: &self.apps_dir,
                state_dir: &self.state_dir,
                runtime_dir: &self.runtime_dir,
                data_dir: &self.data_dir,
                logs_dir: &self.logs_dir,
                tmp_dir: self.tmp_dir.as_deref(),
            },
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
        let environment = self.resolve_environment(&path, name).await?;
        spec.env = environment.values;
        spec.env_origin = environment.origin;
        crate::workspace::materialise_workspace(
            &mut spec,
            &self.config.sandbox.forbidden_writable_roots,
        )
        .await
        .map_err(|source| {
            AppsFileError::from(SpecError::Sandbox {
                app: name.to_string(),
                source,
            })
        })?;
        Ok(spec)
    }

    async fn resolve_environment(
        &self,
        service: &Path,
        name: &str,
    ) -> Result<Environment, AppsFileError> {
        let (origin, declared) = match self.decrypt_environment(service).await? {
            Secrets::Opened(text) => (
                EnvOrigin::Encrypted,
                app_values(&parse_env_text(
                    &text.path,
                    self.host_home.as_deref(),
                    &text.body,
                )?),
            ),
            Secrets::Sealed => (EnvOrigin::Sealed, self.plain_environment(service).await?),
            Secrets::Absent => (EnvOrigin::Plain, self.plain_environment(service).await?),
        };
        let values = merge_environment(&[
            &injected_home(self.host_home.as_deref()),
            &self.global_env,
            &declared,
        ]);
        let counted = declared.len();
        log_environment(name, counted, origin.as_str());
        Ok(Environment { origin, values })
    }

    async fn plain_environment(&self, service: &Path) -> Result<Vec<EnvValue>, AppsFileError> {
        let declared = load_env_file(
            &service.with_extension(ENV_FILE_SUFFIX),
            self.host_home.as_deref(),
        )
        .await?;
        Ok(app_values(&declared))
    }

    async fn decrypt_environment(&self, service: &Path) -> Result<Secrets, AppsFileError> {
        let path = service.with_extension(ENC_FILE_SUFFIX);
        if !enc_file_present(&path).await? {
            return Ok(Secrets::Absent);
        }
        if self.config.sops_identity_file.is_empty() {
            log_unopened_secrets(&path.to_string_lossy());
            return Ok(Secrets::Sealed);
        }
        let body = load_enc_file(
            &self.config.sops_program,
            &path,
            &self.config.sops_identity_file,
            &self.config.search_path,
            self.config.sops_timeout_ms,
        )
        .await?;
        Ok(Secrets::Opened(Decrypted {
            path: path.to_string_lossy().into_owned(),
            body,
        }))
    }
}

struct Environment {
    origin: EnvOrigin,
    values: Vec<EnvValue>,
}

enum Secrets {
    Opened(Decrypted),
    Sealed,
    Absent,
}

struct Decrypted {
    path: String,
    body: String,
}

fn injected_home(home: Option<&str>) -> Vec<EnvValue> {
    home.map_or_else(Vec::new, |value| {
        vec![EnvValue::injected(HOME_VARIABLE, value)]
    })
}

fn app_values(declared: &[(String, String)]) -> Vec<EnvValue> {
    scoped(declared, EnvScope::App)
}

fn log_environment(app: &str, entries: usize, origin: &str) {
    tracing::debug!(
        feature = "service",
        action = "load_env",
        app,
        entries,
        origin,
        "pm3 read the environment values that belong to an app",
    );
}

fn log_unopened_secrets(path: &str) {
    tracing::warn!(
        feature = "service",
        action = "load_env",
        path,
        "pm3 left an encrypted environment file closed because pm3.sops_identity_file names no identity, so the app runs without the values it declares",
    );
}

impl SpecResolver for SpecSource {
    async fn prepare(&self, name: &str) -> Result<AppSpec, SpecResolveError> {
        self.resolve_service(name)
            .await
            .map_err(|error| resolve_failure(name, &error))
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
