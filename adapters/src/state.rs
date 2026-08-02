use thiserror::Error;
use tokio::sync::{mpsc, oneshot};
use usecases::{SupervisionFailure, SupervisionOutcome, SupervisionReply, SupervisionRequest};

#[derive(Debug)]
pub struct DaemonCommand {
    pub request: SupervisionRequest,
    pub reply: oneshot::Sender<SupervisionOutcome>,
}

#[derive(Clone, Debug)]
pub struct DaemonHandle {
    commands: mpsc::Sender<DaemonCommand>,
}

#[derive(Debug, Error)]
pub enum DaemonError {
    #[error("cannot reach the pm3 daemon: it is no longer accepting commands")]
    Unavailable,

    #[error("cannot read the pm3 daemon reply: the daemon dropped the request")]
    Dropped,

    #[error(transparent)]
    Failed(#[from] SupervisionFailure),
}

impl DaemonHandle {
    #[must_use]
    pub const fn new(commands: mpsc::Sender<DaemonCommand>) -> Self {
        Self { commands }
    }

    pub async fn send(&self, request: SupervisionRequest) -> Result<SupervisionReply, DaemonError> {
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
