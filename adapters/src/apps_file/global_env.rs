use std::{collections::BTreeMap, path::Path};

use usecases::{EnvScope, EnvValue};

use super::{
    enc_file::{Decryptor, ENC_FILE_SUFFIX, enc_file_present, load_enc_file},
    env_file::{ENV_FILE_SUFFIX, load_env_file, parse_env_text},
    file::AppsFileError,
};
use crate::config::Pm3Config;

pub const GLOBAL_ENV_STEM: &str = "pm3";

const XDG_PREFIX: &str = "XDG_";

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
    let plain = plain_values(&stem, home).await?;
    let opened = open_secrets(config, &stem, home, &xdg_values(&plain)).await?;
    let declared = merge_layers(&plain, opened);
    log_global_env(declared.len());
    Ok(scoped(&declared, EnvScope::Global))
}

fn merge_layers(plain: &[(String, String)], opened: Opened) -> Vec<(String, String)> {
    let Opened::Values(secrets) = opened else {
        return plain.to_vec();
    };
    let mut merged: BTreeMap<&str, &str> = BTreeMap::new();
    for (key, value) in plain {
        merged.insert(key, value);
    }
    for (key, value) in &secrets {
        merged.insert(key, value);
    }
    merged
        .into_iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

fn xdg_values(plain: &[(String, String)]) -> Vec<(String, String)> {
    plain
        .iter()
        .filter(|(key, _value)| key.starts_with(XDG_PREFIX))
        .cloned()
        .collect()
}

async fn plain_values(
    stem: &Path,
    home: Option<&str>,
) -> Result<Vec<(String, String)>, AppsFileError> {
    Ok(load_env_file(&stem.with_extension(ENV_FILE_SUFFIX), home).await?)
}

async fn sidecar_present(path: &Path) -> bool {
    enc_file_present(path).await.unwrap_or(false)
}
async fn open_secrets(
    config: &Pm3Config,
    stem: &Path,
    home: Option<&str>,
    extra: &[(String, String)],
) -> Result<Opened, AppsFileError> {
    let path = stem.with_extension(ENC_FILE_SUFFIX);
    if !sidecar_present(&path).await {
        return Ok(Opened::Absent);
    }
    let declared_identity = !config.sops_identity_file.is_empty();
    let body = match load_enc_file(
        &Decryptor {
            program: &config.sops_program,
            identity: &config.sops_identity_file,
            search_path: &config.search_path,
            timeout_ms: config.sops_timeout_ms,
            extra_env: extra,
        },
        &path,
    )
    .await
    {
        Ok(text) => text,
        Err(error) if declared_identity => return Err(error.into()),
        Err(_error) => {
            log_sealed_global_env(&path.to_string_lossy());
            return Ok(Opened::Sealed);
        }
    };
    let shown = path.to_string_lossy().into_owned();
    Ok(Opened::Values(parse_env_text(&shown, home, &body)?))
}

pub fn scoped(declared: &[(String, String)], scope: EnvScope) -> Vec<EnvValue> {
    declared
        .iter()
        .map(|(key, value)| EnvValue::new(key, value, scope))
        .collect()
}

#[must_use]
pub fn decryptor_env(global: &[EnvValue]) -> Vec<(String, String)> {
    global
        .iter()
        .filter(|entry| entry.key.starts_with(XDG_PREFIX))
        .map(|entry| (entry.key.clone(), entry.value.clone()))
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
