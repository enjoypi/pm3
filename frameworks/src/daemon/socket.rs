use std::path::Path;

use thiserror::Error;
use tokio::net::{UnixListener, UnixStream};

#[derive(Debug)]
pub enum BindOutcome {
    Bound(UnixListener),
    AlreadyRunning,
}

#[derive(Debug, Error)]
pub enum SocketError {
    #[error("cannot remove the stale pm3 socket '{path}': {reason}")]
    Cleanup { path: String, reason: String },

    #[error("cannot bind the pm3 socket '{path}': {reason}")]
    Bind { path: String, reason: String },
}

pub async fn bind_uds(path: &Path) -> Result<BindOutcome, SocketError> {
    if path.exists() {
        if UnixStream::connect(path).await.is_ok() {
            return Ok(BindOutcome::AlreadyRunning);
        }
        tokio::fs::remove_file(path)
            .await
            .map_err(|e| SocketError::Cleanup {
                path: text(path),
                reason: e.to_string(),
            })?;
    }
    let listener = UnixListener::bind(path).map_err(|e| SocketError::Bind {
        path: text(path),
        reason: e.to_string(),
    })?;
    Ok(BindOutcome::Bound(listener))
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
#[path = "../tests/daemon_socket_tests.rs"]
mod tests;
