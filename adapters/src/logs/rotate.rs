use std::{ffi::OsStr, path::Path};

use usecases::{LogRotateError, LogRotator, RotatedLog, log_paths};

const BACKUP_SUFFIX: &str = ".1";

#[derive(Copy, Clone, Debug, Default)]
pub struct CopyTruncateRotator;

impl LogRotator for CopyTruncateRotator {
    async fn rotate_logs(
        &self,
        logs_dir: &str,
        max_bytes: u64,
    ) -> Result<Vec<RotatedLog>, LogRotateError> {
        let mut entries = tokio::fs::read_dir(logs_dir)
            .await
            .map_err(|e| scan_error(logs_dir, &e))?;
        let mut rotated = Vec::new();
        while let Ok(Some(entry)) = entries.next_entry().await {
            if !is_managed_log(&entry.file_name()) {
                continue;
            }
            match rotate_if_oversized(&entry.path(), max_bytes).await {
                Ok(Some(done)) => rotated.push(done),
                Ok(None) => {}
                Err(error) => log_rotate_failure(&entry.path().to_string_lossy(), &error),
            }
        }
        Ok(rotated)
    }
}

fn is_managed_log(name: &OsStr) -> bool {
    let Some(text) = name.to_str() else {
        return false;
    };
    text.ends_with(log_paths::STDOUT_SUFFIX) || text.ends_with(log_paths::STDERR_SUFFIX)
}

async fn rotate_if_oversized(
    path: &Path,
    max_bytes: u64,
) -> Result<Option<RotatedLog>, std::io::Error> {
    let bytes = tokio::fs::metadata(path).await?.len();
    if bytes <= max_bytes {
        return Ok(None);
    }
    let backup = backup_path(path);
    tokio::fs::copy(path, &backup).await?;
    tokio::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(path)
        .await?;
    Ok(Some(RotatedLog {
        path: path.to_string_lossy().into_owned(),
        bytes,
    }))
}

fn backup_path(path: &Path) -> std::path::PathBuf {
    std::path::PathBuf::from(format!("{}{BACKUP_SUFFIX}", path.to_string_lossy()))
}

fn scan_error(logs_dir: &str, error: &std::io::Error) -> LogRotateError {
    LogRotateError::Scan {
        path: logs_dir.to_string(),
        reason: error.to_string(),
    }
}

fn log_rotate_failure(path: &str, error: &std::io::Error) {
    let reason = error.to_string();
    tracing::warn!(
        feature = "supervisor",
        action = "log_rotate",
        path,
        reason,
        "pm3 could not rotate one service log; it keeps growing",
    );
}

#[cfg(test)]
#[path = "../tests/logs_rotate_tests.rs"]
mod tests;
