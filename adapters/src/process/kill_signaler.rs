use std::process::Output;

use tokio::process::Command;
use usecases::{SignalError, Signaler};

pub const KILL_PROGRAM: &str = "/bin/kill";

const TERMINATE_SIGNAL: &str = "-TERM";
const FORCE_SIGNAL: &str = "-KILL";
const UNKNOWN_EXIT_CODE: i32 = -1;

#[derive(Clone, Debug)]
pub struct KillSignaler {
    program: String,
}

impl KillSignaler {
    #[must_use]
    pub const fn with_program(program: String) -> Self {
        Self { program }
    }

    pub async fn force_kill(&self, pid: u32) -> Result<(), SignalError> {
        self.deliver(FORCE_SIGNAL, pid).await
    }

    async fn deliver(&self, signal: &str, pid: u32) -> Result<(), SignalError> {
        let output = Command::new(&self.program)
            .args([signal, &pid.to_string()])
            .output()
            .await
            .map_err(|e| SignalError::Delivery {
                pid,
                reason: e.to_string(),
            })?;
        let code = output.status.code().unwrap_or(UNKNOWN_EXIT_CODE);
        tracing::debug!(
            pid,
            signal,
            code,
            action = "signal",
            "delivered a signal to a managed process"
        );
        if output.status.success() {
            return Ok(());
        }
        Err(SignalError::Delivery {
            pid,
            reason: describe_refusal(&output),
        })
    }
}

impl Default for KillSignaler {
    fn default() -> Self {
        Self {
            program: KILL_PROGRAM.to_string(),
        }
    }
}

impl Signaler for KillSignaler {
    async fn terminate(&self, pid: u32) -> Result<(), SignalError> {
        self.deliver(TERMINATE_SIGNAL, pid).await
    }
}

fn describe_refusal(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let trimmed = stderr.trim();
    if !trimmed.is_empty() {
        return trimmed.to_string();
    }
    let code = output.status.code().unwrap_or(UNKNOWN_EXIT_CODE);
    format!("kill exited with status {code}")
}

#[cfg(test)]
#[path = "../tests/process_kill_signaler_tests.rs"]
mod tests;
