use std::future::Future;

use thiserror::Error;

#[derive(Debug, Eq, PartialEq, Error)]
pub enum FingerprintError {
    #[error("cannot digest '{path}': {reason}")]
    Read { path: String, reason: String },
}

pub trait Fingerprinter: Send + Sync {
    fn digest(&self, text: &str) -> String;

    fn file_digest(
        &self,
        path: &str,
    ) -> impl Future<Output = Result<String, FingerprintError>> + Send;
}

#[cfg(test)]
#[path = "../tests/ports_fingerprint_tests.rs"]
mod tests;
