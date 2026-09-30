use std::{
    collections::{BTreeMap, HashMap},
    time::Duration,
};

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
use tokio::time::{Instant, sleep};
use usecases::{Liveness, ProcessProbe, ResourceSample};

use super::timed::next_pause;

const BYTES_PER_KIB: u64 = 1024;

#[derive(Clone, Debug)]
pub struct WinProcessProbe {
    poll_interval_ms: u64,
}

impl WinProcessProbe {
    #[must_use]
    pub const fn with_timeout(_timeout_ms: u64, poll_interval_ms: u64) -> Self {
        Self { poll_interval_ms }
    }

    fn step(&self) -> Duration {
        Duration::from_millis(self.poll_interval_ms.max(1))
    }
}

#[derive(Clone, Copy, Debug)]
struct Row {
    parent: Option<u32>,
    memory: u64,
    cpu_ms: u64,
    run_s: u64,
}

async fn off_the_runtime<T: Send + Default + 'static>(work: fn(&[u32]) -> T, pids: &[u32]) -> T {
    let pids = pids.to_vec();
    tokio::task::spawn_blocking(move || work(&pids))
        .await
        .unwrap_or_default()
}

fn read_identities(pids: &[u32]) -> HashMap<u32, Liveness> {
    let targets: Vec<Pid> = pids.iter().copied().map(Pid::from_u32).collect();
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&targets),
        true,
        ProcessRefreshKind::nothing(),
    );
    pids.iter()
        .map(|pid| (*pid, liveness_of(system.process(Pid::from_u32(*pid)))))
        .collect()
}

fn liveness_of(process: Option<&sysinfo::Process>) -> Liveness {
    process.map_or(Liveness::Gone, |found| {
        Liveness::Alive(found.start_time().to_string())
    })
}

fn every_process(kind: ProcessRefreshKind) -> HashMap<u32, Row> {
    let mut system = System::new();
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, kind);
    system
        .processes()
        .iter()
        .map(|(pid, process)| {
            let row = Row {
                parent: process.parent().map(Pid::as_u32),
                memory: process.memory(),
                cpu_ms: process.accumulated_cpu_time(),
                run_s: process.run_time(),
            };
            (pid.as_u32(), row)
        })
        .collect()
}

fn read_samples(pids: &[u32]) -> BTreeMap<u32, ResourceSample> {
    let rows = every_process(ProcessRefreshKind::nothing().with_memory().with_cpu());
    tree_samples(&rows, pids)
}

fn read_tree_members(roots: &[u32]) -> usize {
    let rows = every_process(ProcessRefreshKind::nothing());
    roots
        .iter()
        .map(|root| {
            rows.keys()
                .filter(|pid| descends_from(&rows, **pid, *root))
                .count()
        })
        .sum()
}

fn tree_samples(rows: &HashMap<u32, Row>, roots: &[u32]) -> BTreeMap<u32, ResourceSample> {
    roots
        .iter()
        .filter(|root| rows.contains_key(root))
        .map(|root| (*root, tree_sample(rows, *root)))
        .collect()
}

fn tree_sample(rows: &HashMap<u32, Row>, root: u32) -> ResourceSample {
    let (rss_kib, cpu_tenths) = rows
        .iter()
        .filter(|(pid, _)| descends_from(rows, **pid, root))
        .fold((0_u64, 0_u64), |(rss, cpu), (_, row)| {
            (
                rss.saturating_add(row.memory / BYTES_PER_KIB),
                cpu.saturating_add(cpu_tenths_of(row)),
            )
        });
    ResourceSample {
        rss_kib,
        cpu_tenths: u32::try_from(cpu_tenths).unwrap_or(u32::MAX),
    }
}

fn cpu_tenths_of(row: &Row) -> u64 {
    row.cpu_ms / row.run_s.max(1)
}

fn descends_from(rows: &HashMap<u32, Row>, pid: u32, root: u32) -> bool {
    let mut current = pid;
    for _ in 0..=rows.len() {
        if current == root {
            return true;
        }
        let Some(parent) = rows.get(&current).and_then(|row| row.parent) else {
            return false;
        };
        current = parent;
    }
    false
}

impl ProcessProbe for WinProcessProbe {
    async fn identity(&self, pid: u32) -> Liveness {
        self.identities(&[pid])
            .await
            .remove(&pid)
            .unwrap_or(Liveness::Unreadable)
    }

    async fn identities(&self, pids: &[u32]) -> HashMap<u32, Liveness> {
        off_the_runtime(read_identities, pids).await
    }

    async fn resident_memory(&self, pids: &[u32]) -> BTreeMap<u32, u64> {
        self.resource_usage(pids)
            .await
            .into_iter()
            .map(|(pid, sample)| (pid, sample.rss_kib))
            .collect()
    }

    async fn resource_usage(&self, pids: &[u32]) -> BTreeMap<u32, ResourceSample> {
        off_the_runtime(read_samples, pids).await
    }

    async fn wait_group_gone(&self, pgid: u32, timeout_ms: u64) -> bool {
        let started = Instant::now();
        let budget = Duration::from_millis(timeout_ms);
        loop {
            if off_the_runtime(read_tree_members, &[pgid]).await == 0 {
                return true;
            }
            let Some(pause) = next_pause(started.elapsed(), budget, self.step()) else {
                return false;
            };
            sleep(pause).await;
        }
    }

    async fn wait_gone(&self, pid: u32, timeout_ms: u64) -> Liveness {
        let started = Instant::now();
        let budget = Duration::from_millis(timeout_ms);
        loop {
            let liveness = self.identity(pid).await;
            if matches!(liveness, Liveness::Gone) {
                return liveness;
            }
            let Some(pause) = next_pause(started.elapsed(), budget, self.step()) else {
                return liveness;
            };
            sleep(pause).await;
        }
    }
}

#[cfg(test)]
#[path = "../tests/process_win_probe_tests.rs"]
mod tests;
