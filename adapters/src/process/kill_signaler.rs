use std::time::Duration;

use tokio::{
    process::Command,
    time::{Instant, timeout},
};
use usecases::{SignalError, SignalScope, Signaler};

use crate::exit_status::{describe_refusal, exit_code_of};

#[cfg(unix)]
pub const KILL_PROGRAM: &str = "/bin/kill";
#[cfg(windows)]
pub const KILL_PROGRAM: &str = "taskkill";

const FORCE_SIGNAL: &str = "KILL";
#[cfg(unix)]
const ARGUMENT_TERMINATOR: &str = "--";
const LOWEST_SIGNALABLE_PID: u32 = 2;
const UNSAFE_PID_REASON: &str = "pid is outside the safe range";

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
    pub fn with_stop_signal(stop_signal: String, timeout_ms: u64) -> Self {
        Self::new(KILL_PROGRAM.to_string(), stop_signal, timeout_ms)
    }

    async fn signal(&self, signal: &str, target: &str, pid: u32) -> Result<(), SignalError> {
        let arguments = signal_arguments(signal, target, pid);
        let started = Instant::now();
        let call = Command::new(&self.program).args(&arguments).output();
        let output = timeout(Duration::from_millis(self.timeout_ms), call)
            .await
            .map_err(|_elapsed| self.stalled(pid))?
            .map_err(|e| SignalError::Delivery {
                pid,
                reason: e.to_string(),
            })?;
        let code = exit_code_of(&output.status);
        let duration_ms = started.elapsed().as_millis();
        tracing::debug!(
            feature = "supervisor",
            pid,
            signal,
            target,
            code,
            duration_ms,
            action = "signal",
            "delivered a signal to a managed process"
        );
        if output.status.success() {
            return Ok(());
        }
        Err(SignalError::Delivery {
            pid,
            reason: describe_refusal(&String::from_utf8_lossy(&output.stderr), code),
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

impl Signaler for KillSignaler {
    async fn terminate(&self, pid: u32, scope: SignalScope) -> Result<(), SignalError> {
        let signal = self.stop_signal.clone();
        self.deliver(&signal, pid, scope).await
    }

    async fn force_kill(&self, pid: u32, scope: SignalScope) -> Result<(), SignalError> {
        self.deliver(FORCE_SIGNAL, pid, scope).await
    }

    async fn deliver(&self, signal: &str, pid: u32, scope: SignalScope) -> Result<(), SignalError> {
        if !is_signalable(pid) {
            return Err(SignalError::Delivery {
                pid,
                reason: UNSAFE_PID_REASON.to_string(),
            });
        }
        #[cfg(unix)]
        if scope.reaches_the_group() && self.signal(signal, &group_target(pid), pid).await.is_ok() {
            return Ok(());
        }
        #[cfg(not(unix))]
        let _ = scope;
        self.signal(signal, &pid.to_string(), pid).await
    }
}

#[cfg(unix)]
fn signal_arguments(signal: &str, target: &str, _pid: u32) -> Vec<String> {
    vec![
        format!("-{signal}"),
        ARGUMENT_TERMINATOR.to_string(),
        target.to_string(),
    ]
}

#[cfg(windows)]
fn signal_arguments(_signal: &str, _target: &str, pid: u32) -> Vec<String> {
    vec![
        "/PID".to_string(),
        pid.to_string(),
        "/T".to_string(),
        "/F".to_string(),
    ]
}

fn is_signalable(pid: u32) -> bool {
    pid >= LOWEST_SIGNALABLE_PID && i32::try_from(pid).is_ok()
}

#[cfg(unix)]
fn group_target(pid: u32) -> String {
    format!("-{pid}")
}

#[cfg(test)]
#[path = "../tests/process_kill_signaler_tests.rs"]
mod tests;
