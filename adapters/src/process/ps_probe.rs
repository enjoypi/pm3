use std::{
    collections::{BTreeMap, HashMap, HashSet},
    ops::ControlFlow,
    time::Duration,
};

use tokio::{process::Command, time::Instant};
use usecases::{Liveness, ProcessProbe, ResourceSample};

use super::timed::{CommandOutcome, capture_timed, identity_of, poll_until, wait_gone_with};
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
const GROUP_FORMAT: &str = "pid=,pgid=";
const GROUP_ACTION: &str = "probe_group";
const IDENTITY_ACTION: &str = "probe";
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

    fn step(&self) -> Duration {
        Duration::from_millis(self.poll_interval_ms.max(1))
    }

    async fn grouped_samples(&self, pids: &[u32]) -> BTreeMap<u32, ResourceSample> {
        if pids.is_empty() {
            return BTreeMap::new();
        }
        let joined = join_pids(pids);
        let started = Instant::now();
        let Some(stdout) = self
            .ask_every_process(SAMPLE_FORMAT, &joined, RESOURCE_ACTION)
            .await
        else {
            return BTreeMap::new();
        };
        let sampled = group_totals_for(&parse_group_rows(&stdout), pids);
        log_sample_probe(
            RESOURCE_ACTION,
            &joined,
            sampled.len(),
            started.elapsed().as_millis(),
        );
        sampled
    }

    async fn ask_every_process(&self, format: &str, label: &str, action: &str) -> Option<String> {
        let mut command = Command::new(&self.program);
        command
            .args([WIDE_FLAG, EVERY_PROCESS_FLAG, FORMAT_FLAG, format])
            .env(LOCALE_VARIABLE, FIXED_LOCALE);
        self.read_ps(command, label, action).await
    }

    async fn ask_ps(&self, format: &str, joined: &str, action: &str) -> Option<String> {
        let mut command = Command::new(&self.program);
        command
            .args([WIDE_FLAG, FORMAT_FLAG, format, PID_FLAG])
            .arg(joined)
            .env(LOCALE_VARIABLE, FIXED_LOCALE);
        self.read_ps(command, joined, action).await
    }

    async fn group_is_empty(&self, pgid: u32) -> Option<bool> {
        let label = pgid.to_string();
        let stdout = self
            .ask_every_process(GROUP_FORMAT, &label, GROUP_ACTION)
            .await?;
        Some(count_group_members(&stdout, pgid) == 0)
    }

    async fn read_ps(&self, command: Command, joined: &str, action: &str) -> Option<String> {
        let timed = capture_timed(command, self.timeout_ms).await;
        let duration_ms = timed.duration_ms;
        let output = match timed.outcome {
            CommandOutcome::Stalled => {
                log_stalled_probe(joined, self.timeout_ms, duration_ms, action);
                return None;
            }
            CommandOutcome::SpawnFailed(_) => {
                log_unusable_probe(joined, &self.program, duration_ms, action);
                return None;
            }
            CommandOutcome::Finished(output) => output,
        };
        let code = output.status.code();
        if !output.status.success() && code != Some(NO_SUCH_PROCESS_CODE) {
            let refusal = code.unwrap_or(UNKNOWN_EXIT_CODE);
            log_refused_probe(joined, refusal, duration_ms, action);
            return None;
        }
        Some(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

fn seen_as(report: &PsReport, pid: u32) -> Liveness {
    if let Some(token) = report.tokens.get(&pid) {
        return Liveness::Alive(token.clone());
    }
    if report.garbled || report.tokenless.contains(&pid) {
        return Liveness::Unreadable;
    }
    Liveness::Gone
}

fn unreadable(pids: &[u32]) -> HashMap<u32, Liveness> {
    pids.iter()
        .map(|pid| (*pid, Liveness::Unreadable))
        .collect()
}

fn count_group_members(stdout: &str, pgid: u32) -> usize {
    stdout
        .lines()
        .filter_map(parse_member_row)
        .filter(|member| *member == pgid)
        .count()
}

fn parse_member_row(line: &str) -> Option<u32> {
    let mut fields = line.split_whitespace();
    let _pid = fields.next()?.parse::<u32>().ok()?;
    fields.next()?.parse::<u32>().ok()
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

#[derive(Debug, Default)]
struct PsReport {
    tokens: HashMap<u32, String>,
    tokenless: HashSet<u32>,
    garbled: bool,
}

fn parse_report(stdout: &str) -> PsReport {
    let mut report = PsReport::default();
    for line in stdout.lines() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            continue;
        }
        match parse_row(trimmed) {
            Some((pid, Some(token))) => {
                report.tokens.insert(pid, token);
            }
            Some((pid, None)) => {
                report.tokenless.insert(pid);
            }
            None => report.garbled = true,
        }
    }
    report
}

fn parse_row(line: &str) -> Option<(u32, Option<String>)> {
    let (pid, token) = line.split_once(' ')?;
    let pid = pid.parse::<u32>().ok()?;
    let token = token.trim();
    if token.is_empty() {
        return Some((pid, None));
    }
    Some((pid, Some(token.to_string())))
}

impl ProcessProbe for PsProcessProbe {
    async fn identity(&self, pid: u32) -> Liveness {
        identity_of(self, pid).await
    }

    async fn identities(&self, pids: &[u32]) -> HashMap<u32, Liveness> {
        if pids.is_empty() {
            return HashMap::new();
        }
        let joined = join_pids(pids);
        let started = Instant::now();
        let Some(stdout) = self.ask_ps(BATCH_FORMAT, &joined, IDENTITY_ACTION).await else {
            return unreadable(pids);
        };
        let listed = parse_report(&stdout);
        log_probe(&joined, listed.tokens.len(), started.elapsed().as_millis());
        pids.iter()
            .map(|pid| (*pid, seen_as(&listed, *pid)))
            .collect()
    }

    async fn resident_memory(&self, pids: &[u32]) -> BTreeMap<u32, u64> {
        self.grouped_samples(pids)
            .await
            .into_iter()
            .map(|(pid, sample)| (pid, sample.rss_kib))
            .collect()
    }

    async fn resource_usage(&self, pids: &[u32]) -> BTreeMap<u32, ResourceSample> {
        self.grouped_samples(pids).await
    }

    async fn wait_group_gone(&self, pgid: u32, timeout_ms: u64) -> bool {
        poll_until(timeout_ms, self.step(), || async move {
            match self.group_is_empty(pgid).await {
                Some(false) => ControlFlow::Continue(false),
                Some(true) => ControlFlow::Break(true),
                None => ControlFlow::Break(false),
            }
        })
        .await
    }

    async fn wait_gone(&self, pid: u32, timeout_ms: u64) -> Liveness {
        wait_gone_with(self, pid, timeout_ms, self.step()).await
    }
}

fn log_probe(pids: &str, alive: usize, duration_ms: u128) {
    tracing::debug!(
        feature = "supervisor",
        pids,
        alive,
        duration_ms,
        action = IDENTITY_ACTION,
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

fn log_stalled_probe(pids: &str, timeout_ms: u64, duration_ms: u128, action: &str) {
    tracing::warn!(
        feature = "supervisor",
        pids,
        timeout_ms,
        duration_ms,
        action,
        "pm3 gave up probing because ps did not answer",
    );
}

fn log_unusable_probe(pids: &str, program: &str, duration_ms: u128, action: &str) {
    tracing::warn!(
        feature = "supervisor",
        pids,
        program,
        duration_ms,
        action,
        "pm3 cannot run ps, so it cannot tell whether a process is still alive",
    );
}

fn log_refused_probe(pids: &str, code: i32, duration_ms: u128, action: &str) {
    tracing::warn!(
        feature = "supervisor",
        pids,
        code,
        duration_ms,
        action,
        "ps refused to report, so pm3 cannot tell whether a process is still alive",
    );
}

#[cfg(test)]
#[path = "../tests/process_ps_probe_tests.rs"]
mod tests;
