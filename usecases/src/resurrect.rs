use std::collections::BTreeMap;

use entities::{ProcessStatus, topo_sort};

use crate::{
    Ports, Result,
    fingerprint::render_launch,
    persist::save_table,
    record::ProcessRecord,
    start::{StartKind, StartOutcome, build_launch_spec, start_one},
    table::ProcessTable,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Verdict {
    Adopt,
    Respawn { change: Change, stale: Option<u32> },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Change {
    Unknown,
    Gone,
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
    let mut verdicts: BTreeMap<String, Verdict> = BTreeMap::new();
    for record in &stored {
        if was_supposed_to_run(record) {
            let verdict = judge(record, logs_dir, ports).await;
            verdicts.insert(record.runtime.name.clone(), verdict);
        }
    }

    *table = ProcessTable::from_records(
        stored
            .into_iter()
            .map(|record| forget_unless_adopted(record, &verdicts))
            .collect(),
    );

    let order = topo_sort(&table.dependency_nodes())?;
    let mut outcomes = Vec::with_capacity(verdicts.len());
    for name in order {
        let Some(verdict) = verdicts.get(&name).copied() else {
            continue;
        };
        match verdict {
            Verdict::Adopt => outcomes.push(adopt(table, &name, ports).await),
            Verdict::Respawn { change, stale } => {
                log_respawn(&name, change);
                evict(&name, stale, ports).await;
                outcomes.push(start_one(table, &name, logs_dir, ports).await?);
            }
        }
    }
    save_table(table, ports).await?;
    Ok(outcomes)
}

async fn judge(record: &ProcessRecord, logs_dir: &str, ports: &impl Ports) -> Verdict {
    let (Some(pid), Some(identity)) = (record.runtime.pid, record.runtime.identity.as_ref()) else {
        return respawn(Change::Unknown, None);
    };
    let Some(token) = ports.identity(pid).await else {
        return respawn(Change::Gone, None);
    };
    if token != identity.token {
        return respawn(Change::Reused, None);
    }
    let Ok(launch) = build_launch_spec(&record.spec, logs_dir, ports) else {
        return respawn(Change::Launch, Some(pid));
    };
    if ports.digest(&render_launch(&launch)) != identity.launch_digest {
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
    record.runtime.status.is_running() || record.runtime.status.is_shutting_down()
}

fn forget_unless_adopted(
    mut record: ProcessRecord,
    verdicts: &BTreeMap<String, Verdict>,
) -> ProcessRecord {
    if verdicts.get(&record.runtime.name) != Some(&Verdict::Adopt) {
        record.runtime.mark_exited(ProcessStatus::Stopped);
    }
    record.runtime.pending_restart = false;
    record
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
            Self::Reused => "reused",
            Self::Launch => "launch",
            Self::Binary => "binary",
        }
    }
}

#[cfg(test)]
#[path = "tests/resurrect_tests.rs"]
mod tests;
