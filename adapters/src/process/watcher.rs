use std::{collections::HashMap, path::Path, sync::Arc, time::Duration};

use tokio::sync::{Mutex, oneshot};
use usecases::{ExitOutcome, Liveness};

use super::{ps_probe::PsProcessProbe, tokio_launcher::TokioProcessLauncher};

const UNKNOWN_EXIT: ExitOutcome = ExitOutcome { exit_code: None };

pub async fn wait_until_released(path: &Path, timeout_ms: u64, poll_interval_ms: u64) -> bool {
    let step_ms = poll_interval_ms.max(1);
    let mut waited_ms = 0;
    while path.exists() {
        if waited_ms >= timeout_ms {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(step_ms)).await;
        waited_ms = waited_ms.saturating_add(step_ms);
    }
    true
}

#[derive(Copy, Clone, Debug)]
pub struct PollCadence {
    pub interval_ms: u64,
    pub max_interval_ms: u64,
}

impl PollCadence {
    const fn next_after(self, waited_ms: u64) -> u64 {
        let doubled = waited_ms.saturating_mul(2);
        if doubled > self.max_interval_ms {
            return self.max_interval_ms;
        }
        doubled
    }
}

#[derive(Debug)]
struct Watched {
    token: Option<String>,
    departed: oneshot::Sender<()>,
}

#[derive(Debug, Default)]
struct WatchState {
    polling: bool,
    watched: HashMap<u32, Watched>,
}

#[derive(Debug, Default)]
pub struct AdoptedWatch {
    state: Mutex<WatchState>,
}

impl AdoptedWatch {
    pub async fn until_gone(
        self: &Arc<Self>,
        probe: Arc<PsProcessProbe>,
        pid: u32,
        token: Option<String>,
        cadence: PollCadence,
    ) {
        let (departed, gone) = oneshot::channel();
        let start_poller = {
            let mut state = self.state.lock().await;
            state.watched.insert(pid, Watched { token, departed });
            let idle = !state.polling;
            state.polling = true;
            idle
        };
        if start_poller {
            let watch = Arc::clone(self);
            tokio::spawn(async move { watch.poll_until_all_gone(&probe, cadence).await });
        }
        gone.await.ok();
    }

    #[must_use]
    pub async fn is_idle(&self) -> bool {
        !self.state.lock().await.polling
    }

    async fn poll_until_all_gone(&self, probe: &PsProcessProbe, cadence: PollCadence) {
        let mut step_ms = cadence.interval_ms.max(1);
        while let Some(watched) = self.roster().await {
            tokio::time::sleep(Duration::from_millis(step_ms)).await;
            step_ms = cadence.next_after(step_ms);
            let seen = probe.identities(&watched).await;
            self.release(&seen).await;
        }
    }

    async fn roster(&self) -> Option<Vec<u32>> {
        let mut state = self.state.lock().await;
        if state.watched.is_empty() {
            state.polling = false;
            return None;
        }
        Some(state.watched.keys().copied().collect())
    }

    async fn release(&self, seen: &HashMap<u32, Liveness>) {
        let departed = {
            let mut state = self.state.lock().await;
            let (departed, kept): (HashMap<u32, Watched>, HashMap<u32, Watched>) =
                state.watched.drain().partition(|(pid, entry)| {
                    !still_running(*pid, entry.token.as_deref(), seen.get(pid))
                });
            state.watched = kept;
            departed
        };
        for (_pid, entry) in departed {
            entry.departed.send(()).ok();
        }
    }
}

pub async fn wait_for_exit(
    launcher: &TokioProcessLauncher,
    watch: &Arc<AdoptedWatch>,
    probe: Arc<PsProcessProbe>,
    pid: u32,
    token: Option<String>,
    cadence: PollCadence,
) -> Option<ExitOutcome> {
    if launcher.is_child(pid).await {
        return launcher.wait(pid).await;
    }
    watch.until_gone(probe, pid, token, cadence).await;
    launcher.release(pid).await;
    tracing::debug!(
        pid,
        action = "adopted_exit",
        "an inherited process left; pm3 cannot read its exit code"
    );
    Some(UNKNOWN_EXIT)
}

fn still_running(pid: u32, token: Option<&str>, seen: Option<&Liveness>) -> bool {
    match seen {
        Some(Liveness::Alive(reported)) => holds_the_same_process(pid, token, reported),
        Some(Liveness::Unreadable) => true,
        Some(Liveness::Gone) | None => false,
    }
}

fn holds_the_same_process(pid: u32, token: Option<&str>, seen: &str) -> bool {
    let Some(expected) = token else {
        return true;
    };
    if expected == seen {
        return true;
    }
    log_recycled_pid(pid, expected, seen);
    false
}

fn log_recycled_pid(pid: u32, expected: &str, seen: &str) {
    tracing::warn!(
        pid,
        expected,
        seen,
        action = "adopted_exit",
        "the kernel handed this pid to another process, so pm3 stops watching it",
    );
}

#[cfg(test)]
#[path = "../tests/process_watcher_tests.rs"]
mod tests;
