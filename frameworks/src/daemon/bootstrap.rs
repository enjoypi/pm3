use std::{
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use adapters::{CONFIG_FLAG, DAEMON_SUBCOMMAND, Pm3Config, Pm3Paths};
use tokio::process::Command;

use crate::{Error, Result, client::UdsClient};

#[derive(Clone, Debug)]
pub struct DaemonLaunch<'l> {
    pub paths: &'l Pm3Paths,
    pub config_path: &'l str,
    pub program: PathBuf,
    pub attempts: u32,
    pub interval_ms: u64,
    pub request_timeout_ms: u64,
}

impl<'l> DaemonLaunch<'l> {
    #[must_use]
    pub fn from_config(
        paths: &'l Pm3Paths,
        config_path: &'l str,
        program: PathBuf,
        pm3: &Pm3Config,
    ) -> Self {
        let interval_ms = pm3.daemon_poll_interval_ms.max(1);
        let attempts = u32::try_from(pm3.start_timeout_ms / interval_ms)
            .unwrap_or(u32::MAX)
            .max(1);
        Self {
            paths,
            config_path,
            program,
            attempts,
            interval_ms,
            request_timeout_ms: pm3.request_timeout_ms,
        }
    }
}

pub async fn ensure_daemon_running(launch: &DaemonLaunch<'_>) -> Result<()> {
    let client = UdsClient::new(launch.paths.socket.clone(), launch.request_timeout_ms);
    if client.daemon_is_healthy().await {
        return Ok(());
    }
    if !claim_lock(&launch.paths.lock_file).await {
        return wait_until_ready(&client, launch).await;
    }
    let spawned = spawn_daemon(launch);
    release_lock(&launch.paths.lock_file).await;
    spawned?;
    wait_until_ready(&client, launch).await
}

async fn claim_lock(path: &Path) -> bool {
    tokio::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .await
        .is_ok()
}

async fn release_lock(path: &Path) {
    tokio::fs::remove_file(path).await.ok();
}

#[expect(
    clippy::unwrap_in_result,
    reason = "duplicating a freshly opened file handle cannot fail; expect keeps the region reachable"
)]
fn spawn_daemon(launch: &DaemonLaunch<'_>) -> Result<()> {
    let log = open_for_append(&launch.paths.daemon_log)?;
    let errors = log
        .try_clone()
        .expect("internal error: duplicating a freshly opened log handle is infallible");
    Command::new(&launch.program)
        .arg(DAEMON_SUBCOMMAND)
        .arg(CONFIG_FLAG)
        .arg(launch.config_path)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(errors))
        .spawn()
        .map(|_child| ())
        .map_err(|e| Error::DaemonSpawn {
            reason: e.to_string(),
        })
}

async fn wait_until_ready(client: &UdsClient, launch: &DaemonLaunch<'_>) -> Result<()> {
    for _attempt in 0..launch.attempts {
        if client.daemon_is_healthy().await {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(launch.interval_ms)).await;
    }
    Err(Error::DaemonUnready {
        path: launch.paths.socket.to_string_lossy().into_owned(),
        timeout_ms: u64::from(launch.attempts) * launch.interval_ms,
    })
}

fn open_for_append(path: &Path) -> Result<File> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| Error::Layout {
            path: path.to_string_lossy().into_owned(),
            reason: e.to_string(),
        })
}

#[cfg(test)]
#[path = "../tests/daemon_bootstrap_tests.rs"]
mod tests;
