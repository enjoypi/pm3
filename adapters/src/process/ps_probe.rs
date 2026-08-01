use std::time::Duration;

use tokio::{process::Command, time::timeout};
use usecases::{Liveness, ProcessProbe};

use super::kill_signaler::DEFAULT_COMMAND_TIMEOUT_MS;

pub const PS_PROGRAM: &str = "/bin/ps";

const NO_SUCH_PROCESS_CODE: i32 = 1;
const UNKNOWN_EXIT_CODE: i32 = -1;
const WIDE_FLAG: &str = "-ww";
const FORMAT_FLAG: &str = "-o";
const START_TIME_FORMAT: &str = "lstart=";
const PID_FLAG: &str = "-p";
const LOCALE_VARIABLE: &str = "LC_ALL";
const FIXED_LOCALE: &str = "C";

#[derive(Clone, Debug)]
pub struct PsProcessProbe {
    program: String,
    timeout_ms: u64,
}

impl PsProcessProbe {
    #[must_use]
    pub const fn new(program: String, timeout_ms: u64) -> Self {
        Self {
            program,
            timeout_ms,
        }
    }

    #[must_use]
    pub const fn with_program(program: String) -> Self {
        Self::new(program, DEFAULT_COMMAND_TIMEOUT_MS)
    }

    #[must_use]
    pub fn with_timeout(timeout_ms: u64) -> Self {
        Self::new(PS_PROGRAM.to_string(), timeout_ms)
    }
}

impl Default for PsProcessProbe {
    fn default() -> Self {
        Self::with_program(PS_PROGRAM.to_string())
    }
}

impl ProcessProbe for PsProcessProbe {
    async fn identity(&self, pid: u32) -> Liveness {
        let call = Command::new(&self.program)
            .args([WIDE_FLAG, FORMAT_FLAG, START_TIME_FORMAT, PID_FLAG])
            .arg(pid.to_string())
            .env(LOCALE_VARIABLE, FIXED_LOCALE)
            .output();
        let Ok(finished) = timeout(Duration::from_millis(self.timeout_ms), call).await else {
            log_stalled_probe(pid, self.timeout_ms);
            return Liveness::Unreadable;
        };
        let Ok(output) = finished else {
            log_unusable_probe(pid, &self.program);
            return Liveness::Unreadable;
        };
        let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !output.status.success() {
            return refusal(pid, output.status.code());
        }
        if token.is_empty() {
            return Liveness::Gone;
        }
        tracing::debug!(pid, token, action = "probe", "probed a managed process");
        Liveness::Alive(token)
    }
}

fn refusal(pid: u32, code: Option<i32>) -> Liveness {
    if code == Some(NO_SUCH_PROCESS_CODE) {
        return Liveness::Gone;
    }
    log_refused_probe(pid, code.unwrap_or(UNKNOWN_EXIT_CODE));
    Liveness::Unreadable
}

fn log_stalled_probe(pid: u32, timeout_ms: u64) {
    tracing::warn!(
        pid,
        timeout_ms,
        action = "probe",
        "pm3 gave up probing a process because ps did not answer",
    );
}

fn log_unusable_probe(pid: u32, program: &str) {
    tracing::warn!(
        pid,
        program,
        action = "probe",
        "pm3 cannot run ps, so it cannot tell whether a process is still alive",
    );
}

fn log_refused_probe(pid: u32, code: i32) {
    tracing::warn!(
        pid,
        code,
        action = "probe",
        "ps refused to report on a process, so pm3 cannot tell whether it is still alive",
    );
}

#[cfg(test)]
#[path = "../tests/process_ps_probe_tests.rs"]
mod tests;
