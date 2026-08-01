use std::collections::BTreeMap;

use entities::ProcessStatus;
use futures_util::future::join_all;

use crate::{
    Liveness, Ports, Result, UsecaseError,
    fingerprint::render_identity,
    persist::save_table,
    record::ProcessRecord,
    start::{StartKind, StartOutcome, start_one},
    table::{ProcessTable, dependency_order},
};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Verdict {
    Adopt,
    Settle { stale: Option<u32> },
    Respawn { change: Change, stale: Option<u32> },
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Change {
    Unknown,
    Gone,
    Unreadable,
    Reused,
    Launch,
    Binary,
}

pub async fn resurrect(
    table: &mut ProcessTable,
    logs_dir: &str,
    ports: &impl Ports,
) -> Result<Vec<StartOutcome>> {
    let stored = ports.load().await?;
    let verdicts = judge_all(&stored, ports).await;

    *table = ProcessTable::from_records(
        stored
            .into_iter()
            .map(|record| forget_unless_adopted(record, &verdicts))
            .collect(),
    );

    let order = dependency_order(table, log_unordered_recovery);
    let mut outcomes = Vec::with_capacity(verdicts.len());
    for name in order {
        let Some(verdict) = verdicts.get(&name).copied() else {
            continue;
        };
        match verdict {
            Verdict::Adopt => outcomes.push(adopt(table, &name, ports).await),
            Verdict::Settle { stale } => {
                log_settle(&name);
                evict(&name, stale, ports).await;
            }
            Verdict::Respawn { change, stale } => {
                log_respawn(&name, change);
                evict(&name, stale, ports).await;
                match start_one(table, &name, logs_dir, ports).await {
                    Ok(outcome) => outcomes.push(outcome),
                    Err(error) => log_recovery_failure("respawn", &name, &error),
                }
            }
        }
    }
    if let Err(error) = save_table(table, ports).await {
        log_recovery_failure("persist", "-", &error);
    }
    Ok(outcomes)
}

fn log_unordered_recovery(error: &UsecaseError) {
    log_recovery_failure("order", "-", error);
}

async fn judge_all(stored: &[ProcessRecord], ports: &impl Ports) -> BTreeMap<String, Verdict> {
    let pending: Vec<&ProcessRecord> = stored
        .iter()
        .filter(|record| was_supposed_to_run(record))
        .collect();
    let verdicts = join_all(
        pending
            .iter()
            .map(|record| judge(record, ports))
            .collect::<Vec<_>>(),
    )
    .await;
    pending
        .into_iter()
        .map(|record| record.runtime.name.clone())
        .zip(verdicts)
        .collect()
}

async fn judge(record: &ProcessRecord, ports: &impl Ports) -> Verdict {
    if record.runtime.status.is_shutting_down() {
        return Verdict::Settle {
            stale: record.runtime.pid,
        };
    }
    let (Some(pid), Some(identity)) = (record.runtime.pid, record.runtime.identity.as_ref()) else {
        return respawn(Change::Unknown, record.runtime.pid);
    };
    let token = match ports.identity(pid).await {
        Liveness::Alive(token) => token,
        Liveness::Gone => return respawn(Change::Gone, None),
        Liveness::Unreadable => return respawn(Change::Unreadable, Some(pid)),
    };
    if token != identity.token {
        return respawn(Change::Reused, None);
    }
    if ports.digest(&render_identity(&record.spec)) != identity.launch_digest {
        return respawn(Change::Launch, Some(pid));
    }
    let Ok(binary) = ports.file_digest(&record.spec.script).await else {
        return respawn(Change::Binary, Some(pid));
    };
    if binary != identity.binary_digest {
        return respawn(Change::Binary, Some(pid));
    }
    Verdict::Adopt
}

const fn respawn(change: Change, stale: Option<u32>) -> Verdict {
    Verdict::Respawn { change, stale }
}

async fn evict(name: &str, stale: Option<u32>, ports: &impl Ports) {
    let Some(pid) = stale else {
        return;
    };
    let refused = ports.terminate(pid).await.err().map(|e| e.to_string());
    log_evict(name, pid, refused.as_deref());
}

async fn adopt(table: &mut ProcessTable, name: &str, ports: &impl Ports) -> StartOutcome {
    let (pm_id, pid) = {
        let record = table
            .find_by_name_mut(name)
            .expect("internal error: the topological order only names records the table holds");
        record.runtime.mark_online();
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

const fn was_supposed_to_run(record: &ProcessRecord) -> bool {
    !record.runtime.status.is_settled()
}

fn forget_unless_adopted(
    mut record: ProcessRecord,
    verdicts: &BTreeMap<String, Verdict>,
) -> ProcessRecord {
    let judged = verdicts.get(&record.runtime.name);
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

fn log_evict(app: &str, pid: u32, refused: Option<&str>) {
    let reason = refused.unwrap_or_default();
    tracing::debug!(
        feature = "resurrect",
        action = "evict",
        service = app,
        pid,
        reason,
        "pm3 stopped the stale survivor before starting its replacement",
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

impl Change {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Gone => "gone",
            Self::Unreadable => "unreadable",
            Self::Reused => "reused",
            Self::Launch => "launch",
            Self::Binary => "binary",
        }
    }
}

#[cfg(test)]
#[path = "tests/resurrect_tests.rs"]
mod tests;
