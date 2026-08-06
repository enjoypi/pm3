use std::collections::BTreeMap;

use entities::decide_memory_verdict;

use crate::{
    Result, UsecaseError, record::ProcessView, selector::AppSelector, table::ProcessTable,
};

const STRAY_LABEL: &str = "stray";

#[must_use]
pub fn list_apps(table: &ProcessTable, now_ms: u64) -> Vec<ProcessView> {
    table
        .records()
        .iter()
        .map(|record| record.view(now_ms))
        .collect()
}

pub fn describe_app(
    table: &ProcessTable,
    selector: &AppSelector,
    now_ms: u64,
) -> Result<ProcessView> {
    table
        .find(selector)
        .map(|record| record.view(now_ms))
        .ok_or_else(|| UsecaseError::NotFound(selector.to_string()))
}

#[must_use]
pub fn running_pids(table: &ProcessTable) -> Vec<u32> {
    table
        .records()
        .iter()
        .filter(|record| record.runtime.status.is_running())
        .filter_map(|record| record.runtime.pid)
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryWatch {
    pub name: String,
    pub pid: u32,
    pub limit_kib: u64,
}

#[must_use]
pub fn memory_watch_list(table: &ProcessTable) -> Vec<MemoryWatch> {
    table
        .records()
        .iter()
        .filter(|record| record.runtime.status.is_running())
        .filter_map(|record| {
            Some(MemoryWatch {
                name: record.runtime.name.clone(),
                pid: record.runtime.pid?,
                limit_kib: record.spec.max_memory_kib?,
            })
        })
        .collect()
}

#[must_use]
pub fn breached_memory(watched: &[MemoryWatch], sampled: &BTreeMap<u32, u64>) -> Vec<MemoryBreach> {
    watched
        .iter()
        .filter_map(|watch| {
            let rss_kib = *sampled.get(&watch.pid)?;
            decide_memory_verdict(Some(watch.limit_kib), rss_kib)
                .is_breached()
                .then(|| MemoryBreach {
                    name: watch.name.clone(),
                    rss_kib,
                    limit_kib: watch.limit_kib,
                })
        })
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryBreach {
    pub name: String,
    pub rss_kib: u64,
    pub limit_kib: u64,
}

#[must_use]
pub fn unsettled_count(table: &ProcessTable) -> usize {
    table
        .records()
        .iter()
        .filter(|record| !record.runtime.status.is_settled())
        .count()
}

#[must_use]
pub fn armed_schedule_names(table: &ProcessTable) -> Vec<String> {
    table
        .records()
        .iter()
        .filter(|record| record.spec.schedule.is_some() && record.runtime.schedule_armed)
        .map(|record| record.runtime.name.clone())
        .collect()
}

#[must_use]
pub fn schedule_of(table: &ProcessTable, name: &str) -> Option<String> {
    table
        .find_by_name(name)
        .and_then(|record| record.spec.schedule.clone())
}

#[must_use]
pub fn identity_token_of(table: &ProcessTable, selector: &AppSelector) -> Option<String> {
    table
        .find(selector)
        .and_then(|record| record.runtime.identity.as_ref())
        .map(|identity| identity.token.clone())
}

#[must_use]
pub fn unswept_pids(tracked: &[u32], scheduled: &[u32]) -> Vec<u32> {
    tracked
        .iter()
        .filter(|pid| !scheduled.contains(pid))
        .copied()
        .collect()
}

#[must_use]
pub fn owner_of_pid(table: &ProcessTable, pid: u32) -> (String, Option<String>) {
    table
        .records()
        .iter()
        .find(|record| record.runtime.pid == Some(pid))
        .map_or_else(
            || (format!("{STRAY_LABEL}-{pid}"), None),
            |record| {
                let token = record
                    .runtime
                    .identity
                    .as_ref()
                    .map(|identity| identity.token.clone());
                (record.runtime.name.clone(), token)
            },
        )
}

#[cfg(test)]
#[path = "tests/query_supervision_tests.rs"]
mod supervision_tests;
#[cfg(test)]
#[path = "tests/query_tests.rs"]
mod tests;
