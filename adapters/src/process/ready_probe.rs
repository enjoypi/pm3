use std::{process::Stdio, time::Duration};

use tokio::{process::Command, time::timeout};
use usecases::{Readiness, ReadyProbe, ReadyProber};

#[derive(Debug)]
pub struct HostReadyProber {
    attempt_timeout_ms: u64,
}

impl HostReadyProber {
    #[must_use]
    pub const fn new(attempt_timeout_ms: u64) -> Self {
        Self { attempt_timeout_ms }
    }
}

impl ReadyProber for HostReadyProber {
    async fn check_ready(&self, probe: &ReadyProbe) -> Readiness {
        let budget = Duration::from_millis(self.attempt_timeout_ms);
        match probe {
            ReadyProbe::Tcp { host, port } => check_tcp(host, *port, budget).await,
            ReadyProbe::Exec { command } => check_exec(command, budget).await,
        }
    }
}

async fn check_tcp(host: &str, port: u16, budget: Duration) -> Readiness {
    let started = std::time::Instant::now();
    let result = timeout(budget, tokio::net::TcpStream::connect((host, port))).await;
    log_probe("tcp", started.elapsed().as_millis());
    if matches!(result, Ok(Ok(_stream))) {
        Readiness::Ready
    } else {
        Readiness::Pending
    }
}

async fn check_exec(command: &[String], budget: Duration) -> Readiness {
    let Some(program) = command.first() else {
        return Readiness::Failed("the ready probe has no command".to_string());
    };
    let started = std::time::Instant::now();
    let spawned = Command::new(program)
        .args(command.get(1..).unwrap_or_default())
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn();
    let mut child = match spawned {
        Ok(child) => child,
        Err(error) => return Readiness::Failed(error.to_string()),
    };
    let result = timeout(budget, child.wait()).await;
    log_probe("exec", started.elapsed().as_millis());
    if matches!(result, Ok(Ok(status)) if status.success()) {
        Readiness::Ready
    } else {
        Readiness::Pending
    }
}

fn log_probe(kind: &str, duration_ms: u128) {
    tracing::debug!(
        feature = "supervisor",
        action = "probe_ready",
        kind,
        duration_ms,
        "pm3 checked whether a service is ready",
    );
}

#[cfg(test)]
#[path = "../tests/process_ready_probe_tests.rs"]
mod tests;
