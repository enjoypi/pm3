use std::path::Path;

use usecases::{EnvScope, EnvValue};

use super::{
    enc_file::{ENC_FILE_SUFFIX, enc_file_present, load_enc_file},
    env_file::{ENV_FILE_SUFFIX, load_env_file, parse_env_text},
    file::AppsFileError,
};
use crate::config::Pm3Config;

pub const GLOBAL_ENV_STEM: &str = "pm3";

pub enum Opened {
    Values(Vec<(String, String)>),
    Sealed,
    Absent,
}

pub async fn load_global_env(
    config: &Pm3Config,
    config_root: &Path,
    home: Option<&str>,
) -> Result<Vec<EnvValue>, AppsFileError> {
    let stem = config_root.join(GLOBAL_ENV_STEM);
    let declared = match open_secrets(config, &stem, home).await? {
        Opened::Values(values) => values,
        Opened::Sealed | Opened::Absent => plain_values(&stem, home).await?,
    };
    log_global_env(declared.len());
    Ok(scoped(&declared, EnvScope::Global))
}

async fn plain_values(
    stem: &Path,
    home: Option<&str>,
) -> Result<Vec<(String, String)>, AppsFileError> {
    Ok(load_env_file(&stem.with_extension(ENV_FILE_SUFFIX), home).await?)
}

async fn open_secrets(
    config: &Pm3Config,
    stem: &Path,
    home: Option<&str>,
) -> Result<Opened, AppsFileError> {
    let path = stem.with_extension(ENC_FILE_SUFFIX);
    if !enc_file_present(&path).await? {
        return Ok(Opened::Absent);
    }
    if config.sops_identity_file.is_empty() {
        log_sealed_global_env(&path.to_string_lossy());
        return Ok(Opened::Sealed);
    }
    let body = load_enc_file(
        &config.sops_program,
        &path,
        &config.sops_identity_file,
        &config.search_path,
        config.sops_timeout_ms,
    )
    .await?;
    let shown = path.to_string_lossy().into_owned();
    Ok(Opened::Values(parse_env_text(&shown, home, &body)?))
}

pub fn scoped(declared: &[(String, String)], scope: EnvScope) -> Vec<EnvValue> {
    declared
        .iter()
        .map(|(key, value)| EnvValue::new(key, value, scope))
        .collect()
}

fn log_global_env(entries: usize) {
    tracing::debug!(
        feature = "service",
        action = "load_global_env",
        entries,
        "pm3 read the environment values every app inherits",
    );
}

fn log_sealed_global_env(path: &str) {
    tracing::warn!(
        feature = "service",
        action = "load_global_env",
        path,
        "pm3 left the shared encrypted environment closed because pm3.sops_identity_file names no identity, so every app runs without those values",
    );
}

#[cfg(test)]
#[path = "../tests/apps_file_global_env_tests.rs"]
mod tests;
