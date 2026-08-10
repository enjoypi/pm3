#[cfg(unix)]
use std::os::unix::fs::PermissionsExt as _;
#[cfg(windows)]
use std::time::Duration;
use std::{io::Result as IoResult, path::Path};

use axum::serve::Listener;
use thiserror::Error;
#[cfg(windows)]
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeServer, ServerOptions};
#[cfg(unix)]
use tokio::net::{UnixListener, UnixStream, unix::SocketAddr};

#[cfg(unix)]
use crate::layout::owner_uid_of;
#[cfg(windows)]
use crate::layout::pipe_name_of;

#[cfg(unix)]
const OWNER_ONLY_SOCKET: u32 = 0o600;

#[cfg(unix)]
pub type Pm3Listener = OwnerOnlyListener;
#[cfg(windows)]
pub type Pm3Listener = PipeListener;

#[derive(Debug)]
pub enum BindOutcome {
    Bound(Pm3Listener),
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

#[cfg(unix)]
#[derive(Debug)]
pub struct OwnerOnlyListener {
    inner: UnixListener,
    owner: Option<u32>,
}

#[cfg(windows)]
#[derive(Debug)]
pub struct PipeListener {
    name: String,
    pending: Option<NamedPipeServer>,
    accept_retry_ms: u64,
}

#[cfg(windows)]
impl PipeListener {
    const fn new(name: String, first: NamedPipeServer, accept_retry_ms: u64) -> Self {
        Self {
            name,
            pending: Some(first),
            accept_retry_ms,
        }
    }

    async fn next_instance(&mut self) -> Option<NamedPipeServer> {
        if let Some(pending) = self.pending.take() {
            return Some(pending);
        }
        match ServerOptions::new().create(&self.name) {
            Ok(server) => Some(server),
            Err(error) => {
                log_accept(&self.name, &error.to_string());
                tokio::time::sleep(Duration::from_millis(self.accept_retry_ms)).await;
                None
            }
        }
    }
}

#[cfg(windows)]
impl Listener for PipeListener {
    type Io = NamedPipeServer;
    type Addr = PipeAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            let Some(server) = self.next_instance().await else {
                continue;
            };
            match server.connect().await {
                Ok(()) => return (server, PipeAddr),
                Err(error) => log_accept(&self.name, &error.to_string()),
            }
        }
    }

    fn local_addr(&self) -> IoResult<Self::Addr> {
        Ok(PipeAddr)
    }
}

#[cfg(windows)]
#[derive(Debug)]
pub struct PipeAddr;

#[cfg(windows)]
fn log_accept(name: &str, reason: &str) {
    let pipe = name.to_string();
    tracing::warn!(
        feature = "server",
        action = "accept",
        pipe,
        reason,
        "pm3 daemon failed to accept a named pipe connection and will retry",
    );
}

#[cfg(unix)]
pub async fn bind_uds(path: &Path, _accept_retry_ms: u64) -> Result<BindOutcome, SocketError> {
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

#[cfg(windows)]
pub async fn bind_uds(path: &Path, accept_retry_ms: u64) -> Result<BindOutcome, SocketError> {
    let secret = crate::layout::pipe_secret(path)
        .await
        .map_err(|e| SocketError::Bind {
            path: text(path),
            reason: e.to_string(),
        })?;
    let name = pipe_name_of(path, &secret);
    match ServerOptions::new().first_pipe_instance(true).create(&name) {
        Ok(server) => {
            mark_bound(path).await;
            Ok(BindOutcome::Bound(PipeListener::new(
                name,
                server,
                accept_retry_ms,
            )))
        }
        Err(_) if pipe_is_held(&name) => Ok(BindOutcome::AlreadyRunning),
        Err(error) => Err(SocketError::Bind {
            path: text(path),
            reason: error.to_string(),
        }),
    }
}

#[cfg(windows)]
fn pipe_is_held(name: &str) -> bool {
    ClientOptions::new().open(name).is_ok()
}

#[cfg(windows)]
async fn mark_bound(path: &Path) {
    match tokio::fs::remove_file(path).await {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            log_marker(path, &error.to_string());
            return;
        }
    }
    if let Err(error) = adapters::write_private(path, "").await {
        log_marker(path, &error.to_string());
    }
}

#[cfg(windows)]
fn log_marker(path: &Path, reason: &str) {
    let socket = text(path);
    tracing::warn!(
        feature = "server",
        action = "socket_marker",
        socket,
        reason,
        "pm3 cannot maintain the socket marker file, so shutdown watchers may report early",
    );
}

#[cfg(unix)]
impl OwnerOnlyListener {
    #[must_use]
    pub const fn new(inner: UnixListener, owner: Option<u32>) -> Self {
        Self { inner, owner }
    }
}

#[cfg(unix)]
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

#[cfg(unix)]
async fn restrict_to_owner(path: &Path) -> Result<(), SocketError> {
    let permissions = std::fs::Permissions::from_mode(OWNER_ONLY_SOCKET);
    tokio::fs::set_permissions(path, permissions)
        .await
        .map_err(|e| SocketError::Permissions {
            path: text(path),
            reason: e.to_string(),
        })
}

#[cfg(unix)]
fn socket_owner_of(path: &Path) -> Option<u32> {
    let owner = owner_uid_of(path);
    if owner.is_none() {
        log_unknown_owner(path);
    }
    owner
}

#[cfg(unix)]
fn peer_uid_of(stream: &UnixStream) -> Option<u32> {
    stream.peer_cred().ok().map(|peer| peer.uid())
}

#[cfg(unix)]
const fn admits(peer: Option<u32>, owner: Option<u32>) -> bool {
    let (Some(peer), Some(owner)) = (peer, owner) else {
        return true;
    };
    peer == owner
}

#[cfg(unix)]
fn log_unknown_owner(path: &Path) {
    let socket = text(path);
    tracing::warn!(
        feature = "server",
        action = "read_socket_owner",
        socket,
        "pm3 cannot tell which user owns its socket, so it admits every peer the socket mode lets through",
    );
}

#[cfg(unix)]
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
