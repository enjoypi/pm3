use std::future::Future;

use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchSpec {
    pub name: String,
    pub program: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub env: Vec<(String, String)>,
    pub stdout_path: String,
    pub stderr_path: String,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct LaunchedProcess {
    pub pid: u32,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ExitOutcome {
    pub exit_code: Option<i32>,
}

#[derive(Debug, Eq, PartialEq, Error)]
pub enum LaunchError {
    #[error("cannot spawn app '{app}': {reason}")]
    Spawn { app: String, reason: String },

    #[error("cannot open log file '{path}' for app '{app}': {reason}")]
    LogFile {
        app: String,
        path: String,
        reason: String,
    },
}

impl ExitOutcome {
    #[must_use]
    pub const fn clean(self) -> bool {
        matches!(self.exit_code, Some(0))
    }
}

pub trait ProcessLauncher: Send + Sync {
    fn spawn(
        &self,
        spec: &LaunchSpec,
    ) -> impl Future<Output = Result<LaunchedProcess, LaunchError>> + Send;

    fn adopt(&self, pid: u32) -> impl Future<Output = ()> + Send;
}

#[cfg(test)]
#[path = "../tests/ports_launcher_tests.rs"]
mod tests;
