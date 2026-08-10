use std::{path::Path, time::Duration};

use tokio::{process::Command, time::timeout};

use super::layout::parse_version_output;

pub async fn binary_version(path: &Path, timeout_ms: u64) -> Option<String> {
    let probe = Command::new(path).arg("--version").output();
    let output = timeout(Duration::from_millis(timeout_ms), probe)
        .await
        .ok()?
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_version_output(&String::from_utf8_lossy(&output.stdout)).map(str::to_string)
}

#[cfg(test)]
#[path = "../tests/install_probe_tests.rs"]
mod tests;
