use std::{process::Output, time::Duration};

use tokio::{process::Command, time::timeout};
use usecases::{SignalError, Signaler};

use crate::config::STOP_SIGNAL_TERM;

pub const KILL_PROGRAM: &str = "/bin/kill";
pub const DEFAULT_COMMAND_TIMEOUT_MS: u64 = 5000;

const FORCE_SIGNAL: &str = "KILL";
const ARGUMENT_TERMINATOR: &str = "--";
const UNKNOWN_EXIT_CODE: i32 = -1;

#[derive(Clone, Debug)]
pub struct KillSignaler {
    program: String,
    stop_signal: String,
    timeout_ms: u64,
}

impl KillSignaler {
    #[must_use]
    pub const fn new(program: String, stop_signal: String, timeout_ms: u64) -> Self {
        Self {
            program,
            stop_signal,
            timeout_ms,
        }
    }

    #[must_use]
    pub fn with_program(program: String) -> Self {
        Self::new(
            program,
            STOP_SIGNAL_TERM.to_string(),
            DEFAULT_COMMAND_TIMEOUT_MS,
        )
    }

    #[must_use]
    pub fn with_stop_signal(stop_signal: String, timeout_ms: u64) -> Self {
        Self::new(KILL_PROGRAM.to_string(), stop_signal, timeout_ms)
    }

    pub async fn force_kill(&self, pid: u32) -> Result<(), SignalError> {
        self.deliver(FORCE_SIGNAL, pid).await
    }

    async fn deliver(&self, signal: &str, pid: u32) -> Result<(), SignalError> {
        if self.signal(signal, &group_target(pid), pid).await.is_ok() {
            return Ok(());
        }
        self.signal(signal, &pid.to_string(), pid).await
    }

    async fn signal(&self, signal: &str, target: &str, pid: u32) -> Result<(), SignalError> {
        let flag = format!("-{signal}");
        let call = Command::new(&self.program)
            .args([flag.as_str(), ARGUMENT_TERMINATOR, target])
            .output();
        let output = timeout(Duration::from_millis(self.timeout_ms), call)
            .await
            .map_err(|_elapsed| self.stalled(pid))?
            .map_err(|e| SignalError::Delivery {
                pid,
                reason: e.to_string(),
            })?;
        let code = output.status.code().unwrap_or(UNKNOWN_EXIT_CODE);
        tracing::debug!(
            pid,
            signal,
            target,
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

    fn stalled(&self, pid: u32) -> SignalError {
        SignalError::Delivery {
            pid,
            reason: format!(
                "{} did not answer within {}ms",
                self.program, self.timeout_ms
            ),
        }
    }
}

impl Default for KillSignaler {
    fn default() -> Self {
        Self::with_program(KILL_PROGRAM.to_string())
    }
}

impl Signaler for KillSignaler {
    async fn terminate(&self, pid: u32) -> Result<(), SignalError> {
        let signal = self.stop_signal.clone();
        self.deliver(&signal, pid).await
    }
}

fn group_target(pid: u32) -> String {
    format!("-{pid}")
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
