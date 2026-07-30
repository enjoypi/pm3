use std::{
    collections::{HashMap, HashSet},
    process::Stdio,
};

use tokio::{
    fs::{File, OpenOptions},
    process::{Child, Command},
    sync::Mutex,
};
use usecases::{ExitOutcome, LaunchError, LaunchSpec, LaunchedProcess, ProcessLauncher};

#[derive(Debug, Default)]
pub struct TokioProcessLauncher {
    tracked: Mutex<Tracked>,
}

#[derive(Debug, Default)]
struct Tracked {
    children: HashMap<u32, Child>,
    live: HashSet<u32>,
}

impl TokioProcessLauncher {
    pub async fn tracked_pids(&self) -> Vec<u32> {
        self.tracked.lock().await.live.iter().copied().collect()
    }

    pub async fn wait(&self, pid: u32) -> Option<ExitOutcome> {
        let owned = { self.tracked.lock().await.children.remove(&pid) };
        let mut child = owned?;
        let exit_code = child.wait().await.ok().and_then(|status| status.code());
        {
            self.tracked.lock().await.live.remove(&pid);
        }
        tracing::debug!(pid, ?exit_code, action = "wait", "child process exited");
        Some(ExitOutcome { exit_code })
    }
}

impl ProcessLauncher for TokioProcessLauncher {
    async fn spawn(&self, spec: &LaunchSpec) -> Result<LaunchedProcess, LaunchError> {
        let stdout = open_for_append(&spec.name, &spec.stdout_path).await?;
        let stderr = open_for_append(&spec.name, &spec.stderr_path).await?;
        let child = build_command(spec, stdout, stderr)
            .await
            .spawn()
            .map_err(|e| LaunchError::Spawn {
                app: spec.name.clone(),
                reason: e.to_string(),
            })?;
        let pid = child
            .id()
            .expect("internal error: a freshly spawned child always reports a pid");
        {
            let mut tracked = self.tracked.lock().await;
            tracked.live.insert(pid);
            tracked.children.insert(pid, child);
        }
        tracing::debug!(
            app = spec.name,
            pid,
            program = spec.program,
            action = "spawn",
            "child process launched"
        );
        Ok(LaunchedProcess { pid })
    }
}

async fn build_command(spec: &LaunchSpec, stdout: File, stderr: File) -> Command {
    let mut command = Command::new(&spec.program);
    command
        .args(&spec.args)
        .current_dir(&spec.cwd)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout.into_std().await))
        .stderr(Stdio::from(stderr.into_std().await));
    for (key, value) in &spec.env {
        command.env(key, value);
    }
    command
}

async fn open_for_append(app: &str, path: &str) -> Result<File, LaunchError> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await
        .map_err(|e| LaunchError::LogFile {
            app: app.to_string(),
            path: path.to_string(),
            reason: e.to_string(),
        })
}

#[cfg(test)]
#[path = "../test_helpers/process_tokio_launcher_test_helpers.rs"]
mod test_helpers;
#[cfg(test)]
#[path = "../tests/process_tokio_launcher_tests.rs"]
mod tests;
