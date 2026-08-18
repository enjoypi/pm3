use std::{
    fs::File,
    path::{Path, PathBuf},
    process::Stdio,
    time::{Duration, SystemTime},
};

use adapters::{CONFIG_FLAG, DAEMON_SUBCOMMAND, Pm3Config, Pm3Paths, append_private_blocking};
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

    #[must_use]
    pub const fn budget_ms(&self) -> u64 {
        (self.attempts as u64).saturating_mul(self.interval_ms)
    }
}

pub async fn ensure_daemon_running(launch: &DaemonLaunch<'_>) -> Result<()> {
    let client = UdsClient::new(launch.paths.socket.clone(), launch.request_timeout_ms);
    if client.daemon_is_healthy().await {
        return Ok(());
    }
    if !claim_lock(&launch.paths.lock_file, launch.budget_ms()).await {
        return wait_until_ready(&client, launch).await;
    }
    let settled = match spawn_daemon(launch) {
        Ok(()) => wait_until_ready(&client, launch).await,
        Err(error) => Err(error),
    };
    release_lock(&launch.paths.lock_file).await;
    settled
}

async fn claim_lock(path: &Path, stale_after_ms: u64) -> bool {
    if take_lock(path).await {
        return true;
    }
    if !is_abandoned(path, stale_after_ms).await {
        return false;
    }
    log_abandoned_lock(path, stale_after_ms);
    release_lock(path).await;
    take_lock(path).await
}

async fn take_lock(path: &Path) -> bool {
    let mut options = tokio::fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    options.mode(adapters::OWNER_ONLY_FILE);
    options.open(path).await.is_ok()
}

async fn is_abandoned(path: &Path, stale_after_ms: u64) -> bool {
    lock_age(path)
        .await
        .is_some_and(|age| age > Duration::from_millis(stale_after_ms))
}

async fn lock_age(path: &Path) -> Option<Duration> {
    let modified = tokio::fs::metadata(path)
        .await
        .ok()?
        .modified()
        .expect("internal error: the unix filesystems pm3 runs on all record a modified time");
    SystemTime::now().duration_since(modified).ok()
}

async fn release_lock(path: &Path) {
    crate::layout::remove_runtime_file(path).await;
}

fn log_abandoned_lock(path: &Path, stale_after_ms: u64) {
    let lock = path.to_string_lossy();
    tracing::warn!(
        feature = "lifecycle",
        action = "claim_lock",
        lock = %lock,
        stale_after_ms,
        "pm3 is clearing a start lock that outlived its spawn budget",
    );
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
    let mut command = Command::new(&launch.program);
    command
        .arg(DAEMON_SUBCOMMAND)
        .arg(CONFIG_FLAG)
        .arg(launch.config_path)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(errors));
    #[cfg(unix)]
    command.process_group(0);
    command
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
        timeout_ms: launch.budget_ms(),
    })
}

fn open_for_append(path: &Path) -> Result<File> {
    append_private_blocking(path).map_err(|e| Error::Layout {
        path: path.to_string_lossy().into_owned(),
        reason: e.to_string(),
    })
}

#[cfg(test)]
#[path = "../tests/daemon_bootstrap_tests.rs"]
mod tests;
