use std::{path::Path, time::Duration};

use usecases::{ExitOutcome, ProcessProbe as _};

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

pub async fn wait_for_exit(
    launcher: &TokioProcessLauncher,
    probe: &PsProcessProbe,
    pid: u32,
    poll_interval_ms: u64,
) -> Option<ExitOutcome> {
    if launcher.is_child(pid).await {
        return launcher.wait(pid).await;
    }
    poll_until_gone(probe, pid, Duration::from_millis(poll_interval_ms)).await;
    launcher.release(pid).await;
    tracing::debug!(
        pid,
        action = "adopted_exit",
        "an inherited process left; pm3 cannot read its exit code"
    );
    Some(UNKNOWN_EXIT)
}

async fn poll_until_gone(probe: &PsProcessProbe, pid: u32, interval: Duration) {
    while probe.identity(pid).await.is_some() {
        tokio::time::sleep(interval).await;
    }
}

#[cfg(test)]
#[path = "../tests/process_watcher_tests.rs"]
mod tests;
