use std::{
    process::Output,
    time::{Duration, Instant},
};

use tokio::{process::Command, time::timeout};

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

#[cfg(test)]
#[path = "../tests/process_timed_tests.rs"]
mod tests;
