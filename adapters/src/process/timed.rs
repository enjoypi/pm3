use std::{
    ops::ControlFlow,
    process::Output,
    time::{Duration, Instant},
};

use tokio::{process::Command, time::timeout};
use usecases::{Liveness, ProcessProbe};

#[derive(Debug)]
pub enum CommandOutcome {
    Stalled,
    SpawnFailed(std::io::Error),
    Finished(Output),
}

#[derive(Debug)]
pub struct Timed {
    pub outcome: CommandOutcome,
    pub duration_ms: u128,
}

pub async fn capture_timed(mut command: Command, timeout_ms: u64) -> Timed {
    let started = Instant::now();
    let outcome = match timeout(Duration::from_millis(timeout_ms), command.output()).await {
        Err(_elapsed) => CommandOutcome::Stalled,
        Ok(Err(error)) => CommandOutcome::SpawnFailed(error),
        Ok(Ok(output)) => CommandOutcome::Finished(output),
    };
    Timed {
        outcome,
        duration_ms: started.elapsed().as_millis(),
    }
}

#[must_use]
pub fn next_pause(elapsed: Duration, budget: Duration, step: Duration) -> Option<Duration> {
    let remaining = budget.saturating_sub(elapsed);
    if remaining.is_zero() {
        return None;
    }
    Some(remaining.min(step))
}

pub async fn poll_until<T, F>(timeout_ms: u64, step: Duration, mut check: impl FnMut() -> F) -> T
where
    F: Future<Output = ControlFlow<T, T>>,
{
    let started = tokio::time::Instant::now();
    let budget = Duration::from_millis(timeout_ms);
    loop {
        let last = match check().await {
            ControlFlow::Break(settled) => return settled,
            ControlFlow::Continue(last) => last,
        };
        let Some(pause) = next_pause(started.elapsed(), budget, step) else {
            return last;
        };
        tokio::time::sleep(pause).await;
    }
}

pub async fn identity_of(probe: &impl ProcessProbe, pid: u32) -> Liveness {
    probe
        .identities(&[pid])
        .await
        .remove(&pid)
        .unwrap_or(Liveness::Unreadable)
}

pub async fn wait_gone_with(
    probe: &impl ProcessProbe,
    pid: u32,
    timeout_ms: u64,
    step: Duration,
) -> Liveness {
    poll_until(timeout_ms, step, || async move {
        let liveness = probe.identity(pid).await;
        if matches!(liveness, Liveness::Gone) {
            return ControlFlow::Break(liveness);
        }
        ControlFlow::Continue(liveness)
    })
    .await
}

#[cfg(test)]
#[path = "../tests/process_timed_tests.rs"]
mod tests;
