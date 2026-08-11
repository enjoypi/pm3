use std::collections::BTreeMap;

use entities::ProcessStatus;
use futures_util::future::join_all;
use judge::{Change, Verdict, judge_all, surviving_pid};

use crate::{
    FingerprintError, Liveness, Ports, Result, SignalScope, StrandedProcess, UsecaseError,
    fingerprint::pid_was_recycled,
    persist::save_table,
    record::ProcessRecord,
    start::{StartKind, StartOutcome, start_one},
    table::{ProcessTable, dependency_order},
};

mod judge;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum PidTrust {
    Kept,
    Lost,
}

const INIT_PID: u32 = 1;

pub async fn resurrect(
    table: &mut ProcessTable,
    logs_dir: &str,
    kill_timeout_ms: u64,
    ports: &impl Ports,
) -> Result<Vec<StartOutcome>> {
    let contents = ports.load().await?;
    let boot = ports.identity(INIT_PID).await.into_token();
    let trust = PidTrust::of(contents.boot.as_deref(), boot.as_deref());
    sweep_stranded(&contents.stranded, trust, kill_timeout_ms, ports).await;
    let stored = contents.records;
    let verdicts = judge_all(&stored, trust, ports).await;

    *table = ProcessTable::from_records(
        stored
            .into_iter()
            .map(|record| forget_unless_adopted(record, &verdicts))
            .collect(),
    );
    table.remember_boot(boot);

    let order = dependency_order(table, log_unordered_recovery);
    let mut outcomes = Vec::with_capacity(verdicts.len());
    evict_all(&order, &verdicts, kill_timeout_ms, ports).await;
    for name in &order {
        let Some((verdict, _expected)) = verdicts.get(name) else {
            continue;
        };
        match verdict {
            Verdict::Adopt => outcomes.push(adopt(table, name, ports).await),
            Verdict::Settle { .. } => {}
            Verdict::Respawn { .. } => match start_one(table, name, logs_dir, ports).await {
                Ok(outcome) => outcomes.push(outcome),
                Err(error) => log_recovery_failure("respawn", name, &error),
            },
        }
    }
    if let Err(error) = save_table(table, ports).await {
        log_recovery_failure("persist", "-", &error);
    }
    Ok(outcomes)
}

struct Eviction<'a> {
    name: &'a str,
    stale: Option<u32>,
    expected: Option<&'a str>,
    scope: SignalScope,
}

async fn evict_all(
    order: &[String],
    verdicts: &BTreeMap<String, (Verdict, Option<String>)>,
    kill_timeout_ms: u64,
    ports: &impl Ports,
) {
    let plans: Vec<Eviction<'_>> = order
        .iter()
        .filter_map(|name| eviction_plan(name, verdicts.get(name)?))
        .collect();
    join_all(plans.iter().map(|plan| {
        evict(
            plan.name,
            plan.stale,
            plan.expected,
            plan.scope,
            kill_timeout_ms,
            ports,
        )
    }))
    .await;
}

fn eviction_plan<'a>(name: &'a str, judged: &'a (Verdict, Option<String>)) -> Option<Eviction<'a>> {
    let (verdict, expected) = judged;
    match verdict {
        Verdict::Adopt => None,
        Verdict::Settle { stale } => {
            log_settle(name);
            Some(Eviction {
                name,
                stale: *stale,
                expected: expected.as_deref(),
                scope: SignalScope::ProcessGroup,
            })
        }
        Verdict::Respawn { change, stale } => {
            log_respawn(name, *change);
            Some(Eviction {
                name,
                stale: *stale,
                expected: expected.as_deref(),
                scope: change.eviction_scope(),
            })
        }
    }
}

fn log_unordered_recovery(error: &UsecaseError) {
    log_recovery_failure("order", "-", error);
}

async fn sweep_stranded(
    stranded: &[StrandedProcess],
    trust: PidTrust,
    kill_timeout_ms: u64,
    ports: &impl Ports,
) {
    let pids: Vec<u32> = stranded.iter().filter_map(|orphan| orphan.pid).collect();
    let observed = ports.identities(&pids).await;
    for orphan in stranded {
        let Some(pid) = surviving_pid(
            &orphan.name,
            orphan.pid,
            orphan.token.as_deref(),
            trust,
            &observed,
        ) else {
            continue;
        };
        log_stranded(&orphan.name, pid);
        evict_pid(
            &orphan.name,
            pid,
            orphan.token.as_deref(),
            SignalScope::ProcessGroup,
            kill_timeout_ms,
            ports,
        )
        .await;
    }
}

async fn evict(
    name: &str,
    stale: Option<u32>,
    expected: Option<&str>,
    scope: SignalScope,
    kill_timeout_ms: u64,
    ports: &impl Ports,
) {
    let Some(pid) = stale else {
        return;
    };
    evict_pid(name, pid, expected, scope, kill_timeout_ms, ports).await;
}

async fn evict_pid(
    name: &str,
    pid: u32,
    expected: Option<&str>,
    scope: SignalScope,
    kill_timeout_ms: u64,
    ports: &impl Ports,
) {
    if !scope.reaches_the_group() {
        log_unverified_evict(name, pid);
    }
    let fresh = ports.identity(pid).await;
    if matches!(fresh, Liveness::Gone) {
        return;
    }
    if pid_was_recycled(&fresh, expected) {
        log_spared_evict(name, pid);
        return;
    }
    let refused = ports
        .terminate(pid, scope)
        .await
        .err()
        .map(|e| e.to_string());
    log_evict(name, pid, refused.as_deref());
    let liveness = ports.wait_gone(pid, kill_timeout_ms).await;
    if matches!(liveness, Liveness::Gone) {
        return;
    }
    if pid_was_recycled(&liveness, expected) {
        log_spared_evict(name, pid);
        return;
    }
    let forced = ports
        .force_kill(pid, scope)
        .await
        .err()
        .map(|e| e.to_string());
    log_force_evict(name, pid, forced.as_deref());
}

async fn adopt(table: &mut ProcessTable, name: &str, ports: &impl Ports) -> StartOutcome {
    let (pm_id, pid) = {
        let record = table
            .find_by_name_mut(name)
            .expect("internal error: the topological order only names records the table holds");
        if record.runtime.status != ProcessStatus::Launching || record.spec.ready_probe.is_none() {
            record.runtime.mark_online();
        }
        let pid = record
            .runtime
            .pid
            .expect("internal error: an adopted service was verified to hold a live pid");
        (record.runtime.pm_id, pid)
    };
    ports.adopt(pid).await;
    log_adopt(name, pid);
    StartOutcome {
        pm_id,
        name: name.to_string(),
        pid: Some(pid),
        kind: StartKind::Adopted,
    }
}

fn forget_unless_adopted(
    mut record: ProcessRecord,
    verdicts: &BTreeMap<String, (Verdict, Option<String>)>,
) -> ProcessRecord {
    let judged = verdicts
        .get(&record.runtime.name)
        .map(|(verdict, _)| verdict);
    if !record.runtime.status.is_settled() && judged != Some(&Verdict::Adopt) {
        record.runtime.mark_exited(ProcessStatus::Stopped);
    }
    record.runtime.pending_restart = false;
    record
}

fn log_settle(app: &str) {
    tracing::debug!(
        feature = "resurrect",
        action = "settle",
        service = app,
        "pm3 finished the stop the previous daemon had started",
    );
}

fn log_adopt(app: &str, pid: u32) {
    tracing::debug!(
        feature = "resurrect",
        action = "adopt",
        service = app,
        pid,
        "pm3 reclaimed a service that outlived the previous daemon",
    );
}

fn log_stranded(app: &str, pid: u32) {
    tracing::warn!(
        feature = "resurrect",
        action = "strand",
        service = app,
        pid,
        "pm3 can no longer read the declaration of a surviving service, so it stops the survivor instead of leaving it unmanaged",
    );
}

fn log_unverified_evict(app: &str, pid: u32) {
    tracing::warn!(
        feature = "resurrect",
        action = "evict_unverified",
        service = app,
        pid,
        "pm3 has no identity token for this survivor, so it signals the single pid instead of the whole process group",
    );
}

fn log_reboot(stored: &str, current: &str) {
    tracing::warn!(
        feature = "resurrect",
        action = "compare_boot",
        stored,
        current,
        "the host booted since pm3 last saved its state, so every recorded pid belongs to someone else now",
    );
}

fn log_rebooted_pid(app: &str, pid: u32) {
    tracing::debug!(
        feature = "resurrect",
        action = "spare_rebooted_pid",
        service = app,
        pid,
        "pm3 leaves a pid from before the reboot alone instead of signalling whatever holds it now",
    );
}

fn log_spared_evict(app: &str, pid: u32) {
    tracing::warn!(
        feature = "resurrect",
        action = "evict",
        service = app,
        pid,
        "pm3 spared a pid the kernel handed to another process",
    );
}

fn log_evict(app: &str, pid: u32, refused: Option<&str>) {
    let Some(reason) = refused else {
        tracing::debug!(
            feature = "resurrect",
            action = "evict",
            service = app,
            pid,
            "pm3 stopped the stale survivor before starting its replacement",
        );
        return;
    };
    tracing::warn!(
        feature = "resurrect",
        action = "evict",
        service = app,
        pid,
        reason,
        "pm3 cannot stop a stale survivor, so it may outlive its replacement",
    );
}

fn log_force_evict(app: &str, pid: u32, refused: Option<&str>) {
    let Some(reason) = refused else {
        tracing::debug!(
            feature = "resurrect",
            action = "force_evict",
            service = app,
            pid,
            "pm3 force killed the stale survivor that ignored the stop signal",
        );
        return;
    };
    tracing::warn!(
        feature = "resurrect",
        action = "force_evict",
        service = app,
        pid,
        reason,
        "pm3 cannot force kill a stale survivor, so it outlives its replacement",
    );
}

fn log_unverifiable_binary(app: &str, error: &FingerprintError) {
    let reason = error.to_string();
    tracing::debug!(
        feature = "resurrect",
        action = "adopt",
        service = app,
        reason,
        "pm3 cannot read the program to verify it, so it keeps the confirmed survivor under watch",
    );
}

fn log_recovery_failure(action: &str, app: &str, error: &UsecaseError) {
    let reason = error.to_string();
    tracing::warn!(
        feature = "resurrect",
        action,
        service = app,
        reason,
        "pm3 cannot finish a recovery step, so it keeps the services it already reclaimed",
    );
}

fn log_respawn(app: &str, change: Change) {
    let reason = change.as_str();
    tracing::debug!(
        feature = "resurrect",
        action = "respawn",
        service = app,
        reason,
        "pm3 cannot reclaim a service, so it starts a fresh one",
    );
}

impl PidTrust {
    fn of(stored: Option<&str>, current: Option<&str>) -> Self {
        let (Some(stored), Some(current)) = (stored, current) else {
            return Self::Kept;
        };
        if stored == current {
            return Self::Kept;
        }
        log_reboot(stored, current);
        Self::Lost
    }
}

#[cfg(test)]
#[path = "tests/resurrect_tests.rs"]
mod tests;
