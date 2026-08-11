use std::collections::{BTreeMap, HashMap};

use super::{PidTrust, log_rebooted_pid, log_spared_evict, log_unverifiable_binary};
use crate::{
    Liveness, Ports, SignalScope,
    fingerprint::{pid_was_recycled, render_identity},
    record::ProcessRecord,
};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(super) enum Verdict {
    Adopt,
    Settle { stale: Option<u32> },
    Respawn { change: Change, stale: Option<u32> },
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(super) enum Change {
    Unknown,
    Gone,
    Unreadable,
    Reused,
    Launch,
    Binary,
    Rebooted,
    Restart,
}

impl Change {
    pub(super) const fn eviction_scope(self) -> SignalScope {
        match self {
            Self::Unknown | Self::Unreadable => SignalScope::SinglePid,
            Self::Gone
            | Self::Reused
            | Self::Launch
            | Self::Binary
            | Self::Rebooted
            | Self::Restart => SignalScope::ProcessGroup,
        }
    }

    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Gone => "gone",
            Self::Unreadable => "unreadable",
            Self::Reused => "reused",
            Self::Launch => "launch",
            Self::Binary => "binary",
            Self::Rebooted => "rebooted",
            Self::Restart => "restart",
        }
    }
}

pub(super) async fn judge_all(
    stored: &[ProcessRecord],
    trust: PidTrust,
    ports: &impl Ports,
) -> BTreeMap<String, (Verdict, Option<String>)> {
    let pending: Vec<&ProcessRecord> = stored
        .iter()
        .filter(|record| was_supposed_to_run(record))
        .collect();
    let pids: Vec<u32> = pending
        .iter()
        .filter_map(|record| record.runtime.pid)
        .collect();
    let observed = ports.identities(&pids).await;
    let verdicts = futures_util::future::join_all(
        pending
            .iter()
            .map(|record| judge(record, trust, &observed, ports))
            .collect::<Vec<_>>(),
    )
    .await;
    pending
        .into_iter()
        .map(|record| record.runtime.name.clone())
        .zip(verdicts)
        .collect()
}

const fn was_supposed_to_run(record: &ProcessRecord) -> bool {
    !record.runtime.status.is_settled()
}

async fn judge(
    record: &ProcessRecord,
    trust: PidTrust,
    observed: &HashMap<u32, Liveness>,
    ports: &impl Ports,
) -> (Verdict, Option<String>) {
    let expected = record
        .runtime
        .identity
        .as_ref()
        .map(|identity| identity.token.clone());
    let verdict = judge_verdict(record, trust, observed, ports).await;
    (verdict, expected)
}

async fn judge_verdict(
    record: &ProcessRecord,
    trust: PidTrust,
    observed: &HashMap<u32, Liveness>,
    ports: &impl Ports,
) -> Verdict {
    if record.runtime.status.is_shutting_down() {
        let stale = surviving_pid(
            &record.runtime.name,
            record.runtime.pid,
            record
                .runtime
                .identity
                .as_ref()
                .map(|identity| identity.token.as_str()),
            trust,
            observed,
        );
        if record.runtime.pending_restart {
            return Verdict::Respawn {
                change: Change::Restart,
                stale,
            };
        }
        return Verdict::Settle { stale };
    }
    if trust == PidTrust::Lost {
        return respawn(Change::Rebooted, None);
    }
    let (Some(pid), Some(identity)) = (record.runtime.pid, record.runtime.identity.as_ref()) else {
        let unverified = surviving_pid(
            &record.runtime.name,
            record.runtime.pid,
            None,
            trust,
            observed,
        );
        return respawn(Change::Unknown, unverified);
    };
    let token = match observed.get(&pid).cloned().unwrap_or(Liveness::Unreadable) {
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
    let binary = match ports.file_digest(&record.spec.script).await {
        Ok(binary) => binary,
        Err(error) => {
            log_unverifiable_binary(&record.runtime.name, &error);
            return Verdict::Adopt;
        }
    };
    if binary != identity.binary_digest {
        return respawn(Change::Binary, Some(pid));
    }
    Verdict::Adopt
}

const fn respawn(change: Change, stale: Option<u32>) -> Verdict {
    Verdict::Respawn { change, stale }
}

pub(super) fn surviving_pid(
    app: &str,
    pid: Option<u32>,
    expected: Option<&str>,
    trust: PidTrust,
    observed: &HashMap<u32, Liveness>,
) -> Option<u32> {
    let pid = pid?;
    if trust == PidTrust::Lost {
        log_rebooted_pid(app, pid);
        return None;
    }
    let liveness = observed.get(&pid).unwrap_or(&Liveness::Unreadable);
    if matches!(liveness, Liveness::Gone) {
        return None;
    }
    if pid_was_recycled(liveness, expected) {
        log_spared_evict(app, pid);
        return None;
    }
    Some(pid)
}
