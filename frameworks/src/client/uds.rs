use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

use adapters::{HEALTH_PATH, REQUEST_ID_HEADER};
use thiserror::Error;
#[cfg(unix)]
use tokio::net::UnixStream;
#[cfg(windows)]
use tokio::net::windows::named_pipe::ClientOptions;
use tokio::{
    io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _},
    time::timeout,
};

#[cfg(windows)]
use crate::layout::pipe_name_of;

pub const OK_STATUS: u16 = 200;

const HOST: &str = "localhost";
const HEADER_TERMINATOR: &str = "\r\n\r\n";
const STATUS_FIELD: usize = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpReply {
    pub status: u16,
    pub body: String,
}

#[derive(Clone, Debug)]
pub struct UdsClient {
    socket: PathBuf,
    timeout_ms: u64,
}

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("cannot connect to the pm3 daemon socket '{path}': {reason}")]
    Connect { path: String, reason: String },

    #[error("cannot send the request to the pm3 daemon on '{path}': {reason}")]
    Send { path: String, reason: String },

    #[error("cannot read the pm3 daemon reply from '{path}': {reason}")]
    Receive { path: String, reason: String },

    #[error("cannot read the pm3 daemon reply from '{path}': the daemon answered nothing")]
    Silent { path: String },

    #[error("cannot read the pm3 daemon reply: {reason}")]
    Malformed { reason: String },

    #[error("cannot get an answer from the pm3 daemon on '{path}' within {timeout_ms} ms")]
    Stalled { path: String, timeout_ms: u64 },
}

impl UdsClient {
    #[must_use]
    pub const fn new(socket: PathBuf, timeout_ms: u64) -> Self {
        Self { socket, timeout_ms }
    }

    pub async fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> Result<HttpReply, ClientError> {
        let request_id = next_request_id();
        let started = Instant::now();
        let raw = self
            .exchange(&http_request(method, path, body, &request_id))
            .await?;
        let reply = parse_http_response(&raw)?;
        let duration_ms = started.elapsed().as_millis();
        tracing::debug!(
            feature = "client",
            request_id,
            method,
            path,
            status = reply.status,
            duration_ms,
            action = "request",
            "pm3 client talked to the daemon",
        );
        Ok(reply)
    }

    pub async fn daemon_is_healthy(&self) -> bool {
        let probed = self.request("GET", HEALTH_PATH, None).await;
        probed.is_ok_and(|reply| reply.status == OK_STATUS)
    }

    async fn exchange(&self, request: &str) -> Result<String, ClientError> {
        timeout(Duration::from_millis(self.timeout_ms), self.talk(request))
            .await
            .map_err(|_elapsed| ClientError::Stalled {
                path: text(&self.socket),
                timeout_ms: self.timeout_ms,
            })?
    }

    async fn talk(&self, request: &str) -> Result<String, ClientError> {
        let mut stream = connect_transport(&self.socket).await?;
        converse(stream.as_mut(), request, &text(&self.socket)).await
    }
}

trait Transport: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Transport for T {}

#[cfg(unix)]
async fn connect_transport(socket: &Path) -> Result<Box<dyn Transport>, ClientError> {
    UnixStream::connect(socket)
        .await
        .map(|stream| Box::new(stream) as Box<dyn Transport>)
        .map_err(|e| ClientError::Connect {
            path: text(socket),
            reason: e.to_string(),
        })
}

#[cfg(windows)]
#[expect(
    clippy::unused_async,
    reason = "named pipe clients open synchronously; the unix twin awaits"
)]
async fn connect_transport(socket: &Path) -> Result<Box<dyn Transport>, ClientError> {
    ClientOptions::new()
        .open(pipe_name_of(socket))
        .map(|stream| Box::new(stream) as Box<dyn Transport>)
        .map_err(|e| ClientError::Connect {
            path: text(socket),
            reason: e.to_string(),
        })
}

async fn converse(
    stream: &mut dyn Transport,
    request: &str,
    path: &str,
) -> Result<String, ClientError> {
    let sent = stream.write_all(request.as_bytes()).await;
    let mut raw = String::new();
    let received = stream.read_to_string(&mut raw).await;
    if !raw.is_empty() {
        return Ok(raw);
    }
    if let Err(error) = sent {
        return Err(ClientError::Send {
            path: path.to_string(),
            reason: error.to_string(),
        });
    }
    if let Err(error) = received {
        return Err(ClientError::Receive {
            path: path.to_string(),
            reason: error.to_string(),
        });
    }
    Err(ClientError::Silent {
        path: path.to_string(),
    })
}

#[must_use]
pub fn next_request_id() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let sequence = NEXT.fetch_add(1, Ordering::Relaxed);
    format!("{}-{sequence}", std::process::id())
}

#[must_use]
pub fn http_request(method: &str, path: &str, body: Option<&str>, request_id: &str) -> String {
    let Some(payload) = body else {
        return format!(
            "{method} {path} HTTP/1.1\r\nHost: {HOST}\r\n{REQUEST_ID_HEADER}: {request_id}\r\nConnection: close{HEADER_TERMINATOR}"
        );
    };
    let length = payload.len();
    format!(
        "{method} {path} HTTP/1.1\r\nHost: {HOST}\r\n{REQUEST_ID_HEADER}: {request_id}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {length}{HEADER_TERMINATOR}{payload}"
    )
}

pub fn parse_http_response(raw: &str) -> Result<HttpReply, ClientError> {
    let Some((head, body)) = raw.split_once(HEADER_TERMINATOR) else {
        return Err(malformed("the headers are not terminated by a blank line"));
    };
    let Some(status_line) = head.lines().next() else {
        return Err(malformed("the status line is missing"));
    };
    let Some(code) = status_line.split_whitespace().nth(STATUS_FIELD) else {
        return Err(malformed("the status line carries no status code"));
    };
    let Ok(status) = code.parse::<u16>() else {
        return Err(malformed(&format!(
            "the status code '{code}' is not a number"
        )));
    };
    Ok(HttpReply {
        status,
        body: body.to_string(),
    })
}

fn malformed(reason: &str) -> ClientError {
    ClientError::Malformed {
        reason: reason.to_string(),
    }
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
#[path = "../tests/client_uds_tests.rs"]
mod tests;
