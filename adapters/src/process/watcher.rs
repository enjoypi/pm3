use std::{path::Path, time::Duration};

use usecases::{ExitOutcome, Liveness, ProcessProbe as _};

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

pub async fn wait_for_exit(
    launcher: &TokioProcessLauncher,
    probe: &PsProcessProbe,
    pid: u32,
    token: Option<String>,
    cadence: PollCadence,
) -> Option<ExitOutcome> {
    if launcher.is_child(pid).await {
        return launcher.wait(pid).await;
    }
    poll_until_gone(probe, pid, token.as_deref(), cadence).await;
    launcher.release(pid).await;
    tracing::debug!(
        pid,
        action = "adopted_exit",
        "an inherited process left; pm3 cannot read its exit code"
    );
    Some(UNKNOWN_EXIT)
}

async fn poll_until_gone(
    probe: &PsProcessProbe,
    pid: u32,
    token: Option<&str>,
    cadence: PollCadence,
) {
    let mut step_ms = cadence.interval_ms.max(1);
    while still_running(probe, pid, token).await {
        tokio::time::sleep(Duration::from_millis(step_ms)).await;
        step_ms = cadence.next_after(step_ms);
    }
}

async fn still_running(probe: &PsProcessProbe, pid: u32, token: Option<&str>) -> bool {
    match probe.identity(pid).await {
        Liveness::Alive(seen) => holds_the_same_process(pid, token, &seen),
        Liveness::Gone => false,
        Liveness::Unreadable => true,
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
