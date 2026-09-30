use std::path::{Path, PathBuf};

use usecases::{EnvScope, EnvValue, merge_environment};

use super::{
    enc_file::{ENC_FILE_SUFFIX, enc_file_present, open_declared},
    env_file::{ENV_FILE_SUFFIX, load_env_file, parse_env_text},
    file::AppsFileError,
};
use crate::config::Pm3Config;

pub const GLOBAL_ENV_STEM: &str = "pm3";

const XDG_PREFIX: &str = "XDG_";

pub async fn load_global_env(
    config: &Pm3Config,
    config_root: &Path,
    home: Option<&str>,
) -> Result<Vec<EnvValue>, AppsFileError> {
    let stem = config_root.join(GLOBAL_ENV_STEM);
    let plain = scoped(&plain_values(&stem, home).await?, EnvScope::Global);
    let secrets = open_secrets(config, &stem, home, &decryptor_env(&plain)).await?;
    let declared = merge_environment(&[&plain, &scoped(&secrets, EnvScope::Global)]);
    log_global_env(declared.len());
    Ok(declared)
}

pub async fn warn_misplaced_global_env(config_root: &Path, cfg_dir: &Path) {
    let Some(stray) = stray_global_env(config_root, cfg_dir) else {
        return;
    };
    for suffix in [ENV_FILE_SUFFIX, ENC_FILE_SUFFIX] {
        let path = stray.with_extension(suffix);
        if tokio::fs::metadata(&path).await.is_ok() {
            log_misplaced_global_env(&path.to_string_lossy(), &config_root.to_string_lossy());
        }
    }
}

fn stray_global_env(config_root: &Path, cfg_dir: &Path) -> Option<PathBuf> {
    if cfg_dir == config_root {
        return None;
    }
    Some(cfg_dir.join(GLOBAL_ENV_STEM))
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
) -> Result<Vec<(String, String)>, AppsFileError> {
    let path = stem.with_extension(ENC_FILE_SUFFIX);
    if !sidecar_present(&path).await {
        return Ok(Vec::new());
    }
    let Some(body) = open_declared(config, &path, extra).await? else {
        log_sealed_global_env(&path.to_string_lossy());
        return Ok(Vec::new());
    };
    let shown = path.to_string_lossy().into_owned();
    Ok(parse_env_text(&shown, home, &body)?)
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
        .map(EnvValue::pair)
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

fn log_misplaced_global_env(path: &str, config_root: &str) {
    tracing::warn!(
        feature = "service",
        action = "load_global_env",
        path,
        config_root,
        "pm3 reads the shared environment from its config root, not from pm3.cfg_dir, so this file is ignored and every app runs without those values",
    );
}

#[cfg(test)]
#[path = "../tests/apps_file_global_env_tests.rs"]
mod tests;
