use std::{
    collections::{BTreeMap, HashMap},
    time::Duration,
};

use tokio::{
    process::Command,
    time::{Instant, sleep},
};
use usecases::{Liveness, ProcessProbe, ResourceSample};

use super::timed::{CommandOutcome, capture_timed};
use crate::exit_status::UNKNOWN_EXIT_CODE;

pub const PS_PROGRAM: &str = "/bin/ps";

const NO_SUCH_PROCESS_CODE: i32 = 1;
const WIDE_FLAG: &str = "-ww";
const FORMAT_FLAG: &str = "-o";
const BATCH_FORMAT: &str = "pid=,lstart=";
const SAMPLE_FORMAT: &str = "pid=,pgid=,rss=,pcpu=";
const PID_SEPARATOR: &str = ",";
const PID_FLAG: &str = "-p";
const EVERY_PROCESS_FLAG: &str = "-A";
const MEMORY_ACTION: &str = "probe_memory";
const RESOURCE_ACTION: &str = "probe_resources";
const EMPTY_SAMPLE: ResourceSample = ResourceSample {
    rss_kib: 0,
    cpu_tenths: 0,
};
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
        self.grouped_samples(pids, MEMORY_ACTION)
            .await
            .into_iter()
            .map(|(pid, sample)| (pid, sample.rss_kib))
            .collect()
    }

    pub async fn resource_samples(&self, pids: &[u32]) -> BTreeMap<u32, ResourceSample> {
        self.grouped_samples(pids, RESOURCE_ACTION).await
    }

    async fn grouped_samples(
        &self,
        pids: &[u32],
        action: &'static str,
    ) -> BTreeMap<u32, ResourceSample> {
        if pids.is_empty() {
            return BTreeMap::new();
        }
        let joined = join_pids(pids);
        let started = Instant::now();
        let Some(stdout) = self.ask_every_process(SAMPLE_FORMAT, &joined).await else {
            return BTreeMap::new();
        };
        let sampled = group_totals_for(&parse_group_rows(&stdout), pids);
        log_sample_probe(action, &joined, sampled.len(), started.elapsed().as_millis());
        sampled
    }

    async fn ask_every_process(&self, format: &str, label: &str) -> Option<String> {
        let mut command = Command::new(&self.program);
        command
            .args([WIDE_FLAG, EVERY_PROCESS_FLAG, FORMAT_FLAG, format])
            .env(LOCALE_VARIABLE, FIXED_LOCALE);
        self.read_ps(command, label).await
    }

    async fn ask_ps(&self, format: &str, joined: &str) -> Option<String> {
        let mut command = Command::new(&self.program);
        command
            .args([WIDE_FLAG, FORMAT_FLAG, format, PID_FLAG])
            .arg(joined)
            .env(LOCALE_VARIABLE, FIXED_LOCALE);
        self.read_ps(command, joined).await
    }

    async fn read_ps(&self, command: Command, joined: &str) -> Option<String> {
        let output = match capture_timed(command, self.timeout_ms).await {
            CommandOutcome::Stalled => {
                log_stalled_probe(joined, self.timeout_ms);
                return None;
            }
            CommandOutcome::SpawnFailed(_) => {
                log_unusable_probe(joined, &self.program);
                return None;
            }
            CommandOutcome::Finished(output) => output,
        };
        let code = output.status.code();
        if !output.status.success() && code != Some(NO_SUCH_PROCESS_CODE) {
            log_refused_probe(joined, code.unwrap_or(UNKNOWN_EXIT_CODE));
            return None;
        }
        Some(String::from_utf8_lossy(&output.stdout).into_owned())
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

#[derive(Clone, Copy)]
struct GroupRow {
    pid: u32,
    pgid: u32,
    sample: ResourceSample,
}

fn parse_group_rows(stdout: &str) -> Vec<GroupRow> {
    stdout.lines().filter_map(parse_group_row).collect()
}

fn parse_group_row(line: &str) -> Option<GroupRow> {
    let mut fields = line.split_whitespace();
    let pid = fields.next()?.parse::<u32>().ok()?;
    let group = fields.next()?.parse::<u32>().ok()?;
    let rss_kib = fields.next()?.parse::<u64>().ok()?;
    let cpu_tenths = parse_tenths(fields.next()?)?;
    Some(GroupRow {
        pid,
        pgid: group,
        sample: ResourceSample {
            rss_kib,
            cpu_tenths,
        },
    })
}

fn group_totals_for(rows: &[GroupRow], targets: &[u32]) -> BTreeMap<u32, ResourceSample> {
    let mut totals: HashMap<u32, ResourceSample> = HashMap::new();
    let mut own: HashMap<u32, ResourceSample> = HashMap::new();
    for row in rows {
        let total = totals.entry(row.pgid).or_insert(EMPTY_SAMPLE);
        total.rss_kib = total.rss_kib.saturating_add(row.sample.rss_kib);
        total.cpu_tenths = total.cpu_tenths.saturating_add(row.sample.cpu_tenths);
        own.insert(row.pid, row.sample);
    }
    targets
        .iter()
        .filter_map(|pid| Some((*pid, sample_for(*pid, &own, &totals)?)))
        .collect()
}

fn sample_for(
    pid: u32,
    own: &HashMap<u32, ResourceSample>,
    totals: &HashMap<u32, ResourceSample>,
) -> Option<ResourceSample> {
    let mine = own.get(&pid).copied()?;
    Some(totals.get(&pid).copied().unwrap_or(mine))
}

fn parse_tenths(raw: &str) -> Option<u32> {
    let (whole, fraction) = raw.split_once('.').unwrap_or((raw, "0"));
    let whole = whole.parse::<u32>().ok()?;
    let tenths = fraction.chars().next()?.to_digit(10)?;
    Some(whole.saturating_mul(10).saturating_add(tenths))
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

    async fn identities(&self, pids: &[u32]) -> HashMap<u32, Liveness> {
        if pids.is_empty() {
            return HashMap::new();
        }
        let joined = join_pids(pids);
        let started = Instant::now();
        let Some(stdout) = self.ask_ps(BATCH_FORMAT, &joined).await else {
            return unreadable(pids);
        };
        let listed = parse_report(&stdout);
        log_probe(&joined, listed.len(), started.elapsed().as_millis());
        pids.iter()
            .map(|pid| (*pid, seen_as(listed.get(pid))))
            .collect()
    }

    async fn resident_memory(&self, pids: &[u32]) -> BTreeMap<u32, u64> {
        self.resident_memory_kib(pids).await
    }

    async fn resource_usage(&self, pids: &[u32]) -> BTreeMap<u32, ResourceSample> {
        self.resource_samples(pids).await
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

fn log_sample_probe(action: &str, pids: &str, sampled: usize, duration_ms: u128) {
    tracing::debug!(
        feature = "supervisor",
        pids,
        sampled,
        duration_ms,
        action,
        "sampled the resource usage of the managed process groups"
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
