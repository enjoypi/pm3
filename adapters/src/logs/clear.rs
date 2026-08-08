use std::path::Path;

use thiserror::Error;

#[derive(Debug, Error)]
#[error("cannot clear log file '{path}': {reason}")]
pub struct LogClearError {
    pub path: String,
    pub reason: String,
}

pub async fn clear_log(path: &Path) -> Result<(), LogClearError> {
    tokio::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(path)
        .await
        .map_err(|error| LogClearError {
            path: path.to_string_lossy().into_owned(),
            reason: error.to_string(),
        })?;
    Ok(())
}

#[cfg(test)]
#[path = "../tests/logs_clear_tests.rs"]
mod tests;
