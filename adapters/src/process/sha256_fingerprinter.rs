use std::fmt::Write as _;

use sha2::{Digest as _, Sha256};
use tokio::fs;
use usecases::{FingerprintError, Fingerprinter};

#[derive(Clone, Copy, Debug, Default)]
pub struct Sha256Fingerprinter;

impl Fingerprinter for Sha256Fingerprinter {
    fn digest(&self, text: &str) -> String {
        hex(&Sha256::digest(text.as_bytes()))
    }

    async fn file_digest(&self, path: &str) -> Result<String, FingerprintError> {
        let bytes = fs::read(path).await.map_err(|e| FingerprintError::Read {
            path: path.to_string(),
            reason: e.to_string(),
        })?;
        Ok(hex(&Sha256::digest(&bytes)))
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut text, byte| {
            let _ = write!(text, "{byte:02x}");
            text
        })
}

#[cfg(test)]
#[path = "../tests/process_sha256_fingerprinter_tests.rs"]
mod tests;
