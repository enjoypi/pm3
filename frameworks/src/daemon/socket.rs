use std::{io::Result as IoResult, os::unix::fs::PermissionsExt as _, path::Path};

use axum::serve::Listener;
use thiserror::Error;
use tokio::net::{UnixListener, UnixStream, unix::SocketAddr};

use crate::layout::owner_uid_of;

const OWNER_ONLY_SOCKET: u32 = 0o600;

#[derive(Debug)]
pub enum BindOutcome {
    Bound(OwnerOnlyListener),
    AlreadyRunning,
}

#[derive(Debug, Error)]
pub enum SocketError {
    #[error("cannot remove the stale pm3 socket '{path}': {reason}")]
    Cleanup { path: String, reason: String },

    #[error("cannot bind the pm3 socket '{path}': {reason}")]
    Bind { path: String, reason: String },

    #[error("cannot restrict the pm3 socket '{path}' to its owner: {reason}")]
    Permissions { path: String, reason: String },
}

#[derive(Debug)]
pub struct OwnerOnlyListener {
    inner: UnixListener,
    owner: Option<u32>,
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
    let owner = socket_owner_of(path);
    restrict_to_owner(path)
        .await
        .map(|()| BindOutcome::Bound(OwnerOnlyListener::new(listener, owner)))
}

impl OwnerOnlyListener {
    #[must_use]
    pub const fn new(inner: UnixListener, owner: Option<u32>) -> Self {
        Self { inner, owner }
    }
}

impl Listener for OwnerOnlyListener {
    type Io = UnixStream;
    type Addr = SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            let (stream, addr) = Listener::accept(&mut self.inner).await;
            let peer = peer_uid_of(&stream);
            if admits(peer, self.owner) {
                return (stream, addr);
            }
            log_refused_peer(peer, self.owner);
        }
    }

    fn local_addr(&self) -> IoResult<Self::Addr> {
        self.inner.local_addr()
    }
}

async fn restrict_to_owner(path: &Path) -> Result<(), SocketError> {
    let permissions = std::fs::Permissions::from_mode(OWNER_ONLY_SOCKET);
    tokio::fs::set_permissions(path, permissions)
        .await
        .map_err(|e| SocketError::Permissions {
            path: text(path),
            reason: e.to_string(),
        })
}

fn socket_owner_of(path: &Path) -> Option<u32> {
    let owner = owner_uid_of(path);
    if owner.is_none() {
        log_unknown_owner(path);
    }
    owner
}

fn peer_uid_of(stream: &UnixStream) -> Option<u32> {
    stream.peer_cred().ok().map(|peer| peer.uid())
}

const fn admits(peer: Option<u32>, owner: Option<u32>) -> bool {
    let (Some(peer), Some(owner)) = (peer, owner) else {
        return true;
    };
    peer == owner
}

fn log_unknown_owner(path: &Path) {
    let socket = text(path);
    tracing::warn!(
        feature = "server",
        action = "read_socket_owner",
        socket,
        "pm3 cannot tell which user owns its socket, so it admits every peer the socket mode lets through",
    );
}

fn log_refused_peer(peer: Option<u32>, owner: Option<u32>) {
    tracing::warn!(
        feature = "server",
        action = "refuse_peer",
        ?peer,
        ?owner,
        "pm3 dropped a connection from another user before it could send a request",
    );
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
#[path = "../tests/daemon_socket_tests.rs"]
mod tests;
