use std::path::Path;

use tokio::process::Command;

use super::layout::parse_version_output;
use crate::process::{CommandOutcome, capture_timed};

pub async fn binary_version(path: &Path, timeout_ms: u64) -> Option<String> {
    let mut probe = Command::new(path);
    probe.arg("--version");
    let CommandOutcome::Finished(output) = capture_timed(probe, timeout_ms).await.outcome else {
        return None;
    };
    if !output.status.success() {
        return None;
    }
    parse_version_output(&String::from_utf8_lossy(&output.stdout)).map(str::to_string)
}

#[cfg(test)]
#[path = "../tests/install_probe_tests.rs"]
mod tests;
