use std::{
    io,
    path::{Path, PathBuf},
    time::Instant,
};

use thiserror::Error;
use tokio::process::Command;
use usecases::{SpecError, validate_app_name};

use crate::{
    exit_status::exit_code_of,
    process::{CommandOutcome, capture_timed},
};

pub const ENC_FILE_SUFFIX: &str = "enc.yaml";
pub const SOPS_PROGRAM: &str = "sops";

const SSH_IDENTITY_VARIABLE: &str = "SOPS_AGE_SSH_PRIVATE_KEY_FILE";
const AGE_IDENTITY_VARIABLE: &str = "SOPS_AGE_KEY_FILE";
const PATH_VARIABLE: &str = "PATH";
const DECRYPT_FLAG: &str = "-d";
const OUTPUT_TYPE_FLAG: &str = "--output-type";
const OUTPUT_TYPE: &str = "dotenv";
const DECRYPT_ACTION: &str = "decrypt_env";

#[derive(Debug, Error)]
pub enum EncFileError {
    #[error("cannot run '{program}' to decrypt '{path}': {reason}")]
    Unavailable {
        program: String,
        path: String,
        reason: String,
    },

    #[error("cannot decrypt '{path}' within {timeout_ms}ms")]
    Stalled { path: String, timeout_ms: u64 },

    #[error("cannot decrypt '{path}': the decryptor exited with status {code}")]
    Refused { path: String, code: i32 },

    #[error("cannot read the decrypted '{path}': the output is not text")]
    Unreadable { path: String },

    #[error("cannot look at the encrypted environment file '{path}': {reason}")]
    Unreachable { path: String, reason: String },
}

impl EncFileError {
    fn path(&self) -> &str {
        match self {
            Self::Unavailable { path, .. }
            | Self::Stalled { path, .. }
            | Self::Refused { path, .. }
            | Self::Unreadable { path }
            | Self::Unreachable { path, .. } => path,
        }
    }
}

pub fn enc_file_of(cfg_dir: &Path, name: &str) -> Result<PathBuf, SpecError> {
    validate_app_name(name)?;
    Ok(cfg_dir.join(format!("{name}.{ENC_FILE_SUFFIX}")))
}

pub async fn enc_file_present(path: &Path) -> Result<bool, EncFileError> {
    let started = Instant::now();
    let seen = tokio::fs::metadata(path).await;
    let duration_ms = started.elapsed().as_millis();
    match seen {
        Ok(_entry) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(refused(
            EncFileError::Unreachable {
                path: path.to_string_lossy().into_owned(),
                reason: error.to_string(),
            },
            duration_ms,
        )),
    }
}

pub struct Decryptor<'d> {
    pub program: &'d str,
    pub identity: &'d str,
    pub search_path: &'d str,
    pub timeout_ms: u64,
    pub extra_env: &'d [(String, String)],
}

pub async fn load_enc_file(decryptor: &Decryptor<'_>, path: &Path) -> Result<String, EncFileError> {
    let shown = path.to_string_lossy().into_owned();
    let mut command = Command::new(decryptor.program);
    command.env_clear();
    for (key, value) in decryptor.extra_env {
        command.env(key, value);
    }
    command
        .env(SSH_IDENTITY_VARIABLE, decryptor.identity)
        .env(AGE_IDENTITY_VARIABLE, decryptor.identity)
        .env(PATH_VARIABLE, decryptor.search_path)
        .arg(DECRYPT_FLAG)
        .arg(OUTPUT_TYPE_FLAG)
        .arg(OUTPUT_TYPE)
        .arg(path);
    let started = Instant::now();
    let outcome = capture_timed(command, decryptor.timeout_ms).await;
    let duration_ms = started.elapsed().as_millis();
    let output = match outcome {
        CommandOutcome::Stalled => {
            return Err(refused(
                EncFileError::Stalled {
                    path: shown,
                    timeout_ms: decryptor.timeout_ms,
                },
                duration_ms,
            ));
        }
        CommandOutcome::SpawnFailed(error) => {
            return Err(refused(
                EncFileError::Unavailable {
                    program: decryptor.program.to_string(),
                    path: shown,
                    reason: error.to_string(),
                },
                duration_ms,
            ));
        }
        CommandOutcome::Finished(output) => output,
    };
    if !output.status.success() {
        return Err(refused(
            EncFileError::Refused {
                path: shown,
                code: exit_code_of(&output.status),
            },
            duration_ms,
        ));
    }
    let bytes = output.stdout.len();
    match String::from_utf8(output.stdout) {
        Ok(text) => {
            log_decrypted(&shown, duration_ms, bytes);
            Ok(text)
        }
        Err(_error) => Err(refused(
            EncFileError::Unreadable { path: shown },
            duration_ms,
        )),
    }
}

fn refused(error: EncFileError, duration_ms: u128) -> EncFileError {
    let path = error.path().to_string();
    let reason = error.to_string();
    tracing::warn!(
        feature = "service",
        action = DECRYPT_ACTION,
        path,
        duration_ms,
        reason,
        "pm3 cannot read the encrypted environment that belongs to an app, so the app keeps none of the values it declares",
    );
    error
}

fn log_decrypted(path: &str, duration_ms: u128, bytes: usize) {
    tracing::debug!(
        feature = "service",
        action = DECRYPT_ACTION,
        path,
        duration_ms,
        bytes,
        "pm3 decrypted the environment that belongs to an app",
    );
}

#[cfg(test)]
#[path = "../tests/apps_file_enc_file_tests.rs"]
mod tests;
