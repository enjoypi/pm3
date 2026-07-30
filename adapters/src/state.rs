use thiserror::Error;
use tokio::sync::{mpsc, oneshot};
use usecases::{AppSelector, ProcessView, StartOutcome, UsecaseError};

use crate::apps_file::AppsFileError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DaemonRequest {
    Start { apps_file: String },
    List,
    Describe(AppSelector),
    Stop(AppSelector),
    Restart(AppSelector),
    Delete(AppSelector),
    StopAll,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DaemonReply {
    Started(Vec<StartOutcome>),
    Listed(Vec<ProcessView>),
    Described(ProcessView),
    Stopped { name: String },
    Restarted { name: String },
    Deleted { name: String },
    StoppedAll { names: Vec<String> },
}

#[derive(Debug)]
pub struct DaemonCommand {
    pub request: DaemonRequest,
    pub reply: oneshot::Sender<DaemonOutcome>,
}

#[derive(Clone, Debug)]
pub struct DaemonHandle {
    commands: mpsc::Sender<DaemonCommand>,
}

#[derive(Debug, Error)]
pub enum DaemonFailure {
    #[error(transparent)]
    Usecase(#[from] UsecaseError),

    #[error(transparent)]
    Apps(#[from] AppsFileError),
}

#[derive(Debug, Error)]
pub enum DaemonError {
    #[error("cannot reach the pm3 daemon: it is no longer accepting commands")]
    Unavailable,

    #[error("cannot read the pm3 daemon reply: the daemon dropped the request")]
    Dropped,

    #[error(transparent)]
    Failed(#[from] DaemonFailure),
}

pub type DaemonOutcome = Result<DaemonReply, DaemonFailure>;

impl DaemonHandle {
    #[must_use]
    pub const fn new(commands: mpsc::Sender<DaemonCommand>) -> Self {
        Self { commands }
    }

    pub async fn send(&self, request: DaemonRequest) -> Result<DaemonReply, DaemonError> {
        let (reply, answer) = oneshot::channel();
        self.commands
            .send(DaemonCommand { request, reply })
            .await
            .map_err(|_closed| DaemonError::Unavailable)?;
        answer
            .await
            .map_err(|_dropped| DaemonError::Dropped)?
            .map_err(DaemonError::Failed)
    }
}

#[cfg(test)]
#[path = "tests/state_tests.rs"]
mod tests;
