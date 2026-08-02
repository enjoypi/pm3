use std::{collections::HashMap, time::Duration};

use tokio::{
    process::Command,
    time::{Instant, sleep, timeout},
};
use usecases::{Liveness, ProcessProbe};

pub const PS_PROGRAM: &str = "/bin/ps";

const DEFAULT_POLL_INTERVAL_MS: u64 = 50;

const NO_SUCH_PROCESS_CODE: i32 = 1;
const UNKNOWN_EXIT_CODE: i32 = -1;
const WIDE_FLAG: &str = "-ww";
const FORMAT_FLAG: &str = "-o";
const BATCH_FORMAT: &str = "pid=,lstart=";
const PID_SEPARATOR: &str = ",";
const PID_FLAG: &str = "-p";
const LOCALE_VARIABLE: &str = "LC_ALL";
const FIXED_LOCALE: &str = "C";

#[derive(Clone, Debug)]
pub struct PsProcessProbe {
    program: String,
    timeout_ms: u64,
    poll_interval_ms: u64,
}

impl PsProcessProbe {
    #[must_use]
    pub const fn new(program: String, timeout_ms: u64) -> Self {
        Self {
            program,
            timeout_ms,
            poll_interval_ms: DEFAULT_POLL_INTERVAL_MS,
        }
    }

    #[must_use]
    pub fn with_timeout(timeout_ms: u64) -> Self {
        Self::new(PS_PROGRAM.to_string(), timeout_ms)
    }

    #[must_use]
    pub const fn with_poll_interval(mut self, poll_interval_ms: u64) -> Self {
        self.poll_interval_ms = poll_interval_ms;
        self
    }

    pub async fn identities(&self, pids: &[u32]) -> HashMap<u32, Liveness> {
        if pids.is_empty() {
            return HashMap::new();
        }
        let joined = pids
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(PID_SEPARATOR);
        let call = Command::new(&self.program)
            .args([WIDE_FLAG, FORMAT_FLAG, BATCH_FORMAT, PID_FLAG])
            .arg(&joined)
            .env(LOCALE_VARIABLE, FIXED_LOCALE)
            .output();
        let Ok(finished) = timeout(Duration::from_millis(self.timeout_ms), call).await else {
            log_stalled_probe(&joined, self.timeout_ms);
            return unreadable(pids);
        };
        let Ok(output) = finished else {
            log_unusable_probe(&joined, &self.program);
            return unreadable(pids);
        };
        let code = output.status.code();
        if !output.status.success() && code != Some(NO_SUCH_PROCESS_CODE) {
            log_refused_probe(&joined, code.unwrap_or(UNKNOWN_EXIT_CODE));
            return unreadable(pids);
        }
        let listed = parse_report(&String::from_utf8_lossy(&output.stdout));
        log_probe(&joined, listed.len());
        pids.iter()
            .map(|pid| (*pid, seen_as(listed.get(pid))))
            .collect()
    }
}

fn seen_as(token: Option<&String>) -> Liveness {
    token.map_or(Liveness::Gone, |seen| Liveness::Alive(seen.clone()))
}

fn unreadable(pids: &[u32]) -> HashMap<u32, Liveness> {
    pids.iter()
        .map(|pid| (*pid, Liveness::Unreadable))
        .collect()
}

fn parse_report(stdout: &str) -> HashMap<u32, String> {
    stdout
        .lines()
        .filter_map(|line| {
            let (pid, token) = line.trim_start().split_once(' ')?;
            let token = token.trim();
            if token.is_empty() {
                return None;
            }
            Some((pid.parse::<u32>().ok()?, token.to_string()))
        })
        .collect()
}

impl ProcessProbe for PsProcessProbe {
    async fn identity(&self, pid: u32) -> Liveness {
        self.identities(&[pid])
            .await
            .remove(&pid)
            .unwrap_or(Liveness::Unreadable)
    }

    async fn wait_gone(&self, pid: u32, timeout_ms: u64) -> Liveness {
        let started = Instant::now();
        let budget = Duration::from_millis(timeout_ms);
        loop {
            let liveness = self.identity(pid).await;
            if matches!(liveness, Liveness::Gone) {
                return liveness;
            }
            let remaining = budget.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                return liveness;
            }
            let step = Duration::from_millis(self.poll_interval_ms.max(1));
            sleep(remaining.min(step)).await;
        }
    }
}

fn log_probe(pids: &str, alive: usize) {
    tracing::debug!(
        pids,
        alive,
        action = "probe",
        "probed the managed processes"
    );
}

fn log_stalled_probe(pids: &str, timeout_ms: u64) {
    tracing::warn!(
        pids,
        timeout_ms,
        action = "probe",
        "pm3 gave up probing because ps did not answer",
    );
}

fn log_unusable_probe(pids: &str, program: &str) {
    tracing::warn!(
        pids,
        program,
        action = "probe",
        "pm3 cannot run ps, so it cannot tell whether a process is still alive",
    );
}

fn log_refused_probe(pids: &str, code: i32) {
    tracing::warn!(
        pids,
        code,
        action = "probe",
        "ps refused to report, so pm3 cannot tell whether a process is still alive",
    );
}

#[cfg(test)]
#[path = "../tests/process_ps_probe_tests.rs"]
mod tests;
