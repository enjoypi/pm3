use std::future::Future;

use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RotatedLog {
    pub path: String,
    pub bytes: u64,
}

#[derive(Debug, Eq, PartialEq, Error)]
pub enum LogRotateError {
    #[error("cannot scan the log directory '{path}': {reason}")]
    Scan { path: String, reason: String },
}

pub trait LogRotator: Send + Sync {
    fn rotate_logs(
        &self,
        logs_dir: &str,
        max_bytes: u64,
    ) -> impl Future<Output = Result<Vec<RotatedLog>, LogRotateError>> + Send;
}

#[cfg(test)]
#[path = "../tests/ports_log_rotate_tests.rs"]
mod tests;
