use tokio::{process::Command, time::Instant};
use usecases::{SignalError, SignalScope, Signaler};

use super::timed::{CommandOutcome, capture_timed};
use crate::exit_status::{describe_refusal, exit_code_of};

#[cfg(unix)]
pub const KILL_PROGRAM: &str = "/bin/kill";

const FORCE_SIGNAL: &str = "KILL";
#[cfg(unix)]
const ARGUMENT_TERMINATOR: &str = "--";
const LOWEST_SIGNALABLE_PID: u32 = 2;
const UNSAFE_PID_REASON: &str = "pid is outside the safe range";
const SIGNAL_ACTION: &str = "signal";

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
    pub fn with_stop_signal(stop_signal: String, timeout_ms: u64, taskkill_path: &str) -> Self {
        Self::new(kill_program(taskkill_path), stop_signal, timeout_ms)
    }

    async fn signal(&self, signal: &str, target: &str, pid: u32) -> Result<(), SignalError> {
        let arguments = signal_arguments(signal, target, pid);
        let started = Instant::now();
        let mut command = Command::new(&self.program);
        command.args(&arguments);
        let output = match capture_timed(command, self.timeout_ms).await {
            CommandOutcome::Stalled => {
                let refusal = self.stalled(pid);
                log_undelivered_signal(pid, signal, target, elapsed_ms(started), &refusal);
                return Err(refusal);
            }
            CommandOutcome::SpawnFailed(error) => {
                let refusal = SignalError::Delivery {
                    pid,
                    reason: error.to_string(),
                };
                log_undelivered_signal(pid, signal, target, elapsed_ms(started), &refusal);
                return Err(refusal);
            }
            CommandOutcome::Finished(output) => output,
        };
        let code = exit_code_of(&output.status);
        let duration_ms = elapsed_ms(started);
        tracing::debug!(
            feature = "supervisor",
            pid,
            signal,
            target,
            code,
            duration_ms,
            action = SIGNAL_ACTION,
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

#[cfg(unix)]
fn kill_program(_taskkill_path: &str) -> String {
    KILL_PROGRAM.to_string()
}

#[cfg(windows)]
fn kill_program(taskkill_path: &str) -> String {
    taskkill_path.to_string()
}

fn elapsed_ms(started: Instant) -> u128 {
    started.elapsed().as_millis()
}

fn log_undelivered_signal(
    pid: u32,
    signal: &str,
    target: &str,
    duration_ms: u128,
    refusal: &SignalError,
) {
    let reason = refusal.to_string();
    tracing::warn!(
        feature = "supervisor",
        pid,
        signal,
        target,
        duration_ms,
        reason,
        action = SIGNAL_ACTION,
        "pm3 never delivered a signal, so the process may outlive its service"
    );
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
