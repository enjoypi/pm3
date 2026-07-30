use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use thiserror::Error;
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::UnixStream,
    time::timeout,
};

pub const HEALTH_PATH: &str = "/health";
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
        let raw = self.exchange(&http_request(method, path, body)).await?;
        let reply = parse_http_response(&raw)?;
        tracing::debug!(
            method,
            path,
            status = reply.status,
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
        let mut stream =
            UnixStream::connect(&self.socket)
                .await
                .map_err(|e| ClientError::Connect {
                    path: text(&self.socket),
                    reason: e.to_string(),
                })?;
        stream.write_all(request.as_bytes()).await.ok();
        let mut raw = String::new();
        stream.read_to_string(&mut raw).await.ok();
        if raw.is_empty() {
            return Err(ClientError::Silent {
                path: text(&self.socket),
            });
        }
        Ok(raw)
    }
}

#[must_use]
pub fn http_request(method: &str, path: &str, body: Option<&str>) -> String {
    let Some(payload) = body else {
        return format!(
            "{method} {path} HTTP/1.1\r\nHost: {HOST}\r\nConnection: close{HEADER_TERMINATOR}"
        );
    };
    let length = payload.len();
    format!(
        "{method} {path} HTTP/1.1\r\nHost: {HOST}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {length}{HEADER_TERMINATOR}{payload}"
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
