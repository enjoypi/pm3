use std::{process::Output, time::Duration};

use tokio::{process::Command, time::timeout};

#[derive(Debug)]
pub enum CommandOutcome {
    Stalled,
    SpawnFailed(std::io::Error),
    Finished(Output),
}

pub async fn capture_timed(mut command: Command, timeout_ms: u64) -> CommandOutcome {
    match timeout(Duration::from_millis(timeout_ms), command.output()).await {
        Err(_elapsed) => CommandOutcome::Stalled,
        Ok(Err(error)) => CommandOutcome::SpawnFailed(error),
        Ok(Ok(output)) => CommandOutcome::Finished(output),
    }
}

#[cfg(test)]
#[path = "../tests/process_timed_tests.rs"]
mod tests;
