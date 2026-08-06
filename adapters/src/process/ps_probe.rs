use std::{
    collections::{BTreeMap, HashMap},
    time::Duration,
};

use tokio::{
    process::Command,
    time::{Instant, sleep, timeout},
};
use usecases::{Liveness, ProcessProbe};

pub const PS_PROGRAM: &str = "/bin/ps";

const NO_SUCH_PROCESS_CODE: i32 = 1;
const UNKNOWN_EXIT_CODE: i32 = -1;
const WIDE_FLAG: &str = "-ww";
const FORMAT_FLAG: &str = "-o";
const BATCH_FORMAT: &str = "pid=,lstart=";
const MEMORY_FORMAT: &str = "pid=,rss=";
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
    pub const fn new(program: String, timeout_ms: u64, poll_interval_ms: u64) -> Self {
        Self {
            program,
            timeout_ms,
            poll_interval_ms,
        }
    }

    #[must_use]
    pub fn with_timeout(timeout_ms: u64, poll_interval_ms: u64) -> Self {
        Self::new(PS_PROGRAM.to_string(), timeout_ms, poll_interval_ms)
    }

    pub async fn resident_memory_kib(&self, pids: &[u32]) -> BTreeMap<u32, u64> {
        if pids.is_empty() {
            return BTreeMap::new();
        }
        let joined = join_pids(pids);
        let started = Instant::now();
        let Some(stdout) = self.ask_ps(MEMORY_FORMAT, &joined).await else {
            return BTreeMap::new();
        };
        let listed = parse_memory_report(&stdout);
        log_memory_probe(&joined, listed.len(), started.elapsed().as_millis());
        listed
    }

    async fn ask_ps(&self, format: &str, joined: &str) -> Option<String> {
        let call = Command::new(&self.program)
            .args([WIDE_FLAG, FORMAT_FLAG, format, PID_FLAG])
            .arg(joined)
            .env(LOCALE_VARIABLE, FIXED_LOCALE)
            .output();
        let Ok(finished) = timeout(Duration::from_millis(self.timeout_ms), call).await else {
            log_stalled_probe(joined, self.timeout_ms);
            return None;
        };
        let Ok(output) = finished else {
            log_unusable_probe(joined, &self.program);
            return None;
        };
        let code = output.status.code();
        if !output.status.success() && code != Some(NO_SUCH_PROCESS_CODE) {
            log_refused_probe(joined, code.unwrap_or(UNKNOWN_EXIT_CODE));
            return None;
        }
        Some(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    pub async fn identities(&self, pids: &[u32]) -> HashMap<u32, Liveness> {
        if pids.is_empty() {
            return HashMap::new();
        }
        let joined = join_pids(pids);
        let started = Instant::now();
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
        log_probe(&joined, listed.len(), started.elapsed().as_millis());
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

fn join_pids(pids: &[u32]) -> String {
    pids.iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(PID_SEPARATOR)
}

fn parse_memory_report(stdout: &str) -> BTreeMap<u32, u64> {
    stdout
        .lines()
        .filter_map(|line| {
            let (pid, rss) = line.trim_start().split_once(' ')?;
            Some((pid.parse::<u32>().ok()?, rss.trim().parse::<u64>().ok()?))
        })
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

    async fn resident_memory(&self, pids: &[u32]) -> BTreeMap<u32, u64> {
        self.resident_memory_kib(pids).await
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

fn log_probe(pids: &str, alive: usize, duration_ms: u128) {
    tracing::debug!(
        feature = "supervisor",
        pids,
        alive,
        duration_ms,
        action = "probe",
        "probed the managed processes"
    );
}

fn log_memory_probe(pids: &str, sampled: usize, duration_ms: u128) {
    tracing::debug!(
        feature = "supervisor",
        pids,
        sampled,
        duration_ms,
        action = "probe_memory",
        "sampled the resident memory of the managed processes"
    );
}

fn log_stalled_probe(pids: &str, timeout_ms: u64) {
    tracing::warn!(
        feature = "supervisor",
        pids,
        timeout_ms,
        action = "probe",
        "pm3 gave up probing because ps did not answer",
    );
}

fn log_unusable_probe(pids: &str, program: &str) {
    tracing::warn!(
        feature = "supervisor",
        pids,
        program,
        action = "probe",
        "pm3 cannot run ps, so it cannot tell whether a process is still alive",
    );
}

fn log_refused_probe(pids: &str, code: i32) {
    tracing::warn!(
        feature = "supervisor",
        pids,
        code,
        action = "probe",
        "ps refused to report, so pm3 cannot tell whether a process is still alive",
    );
}

#[cfg(test)]
#[path = "../tests/process_ps_probe_tests.rs"]
mod tests;
