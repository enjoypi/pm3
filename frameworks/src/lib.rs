pub mod cli;
pub mod client;
pub mod commands;
pub mod daemon;
pub mod layout;
pub mod sandbox_probe;
pub mod server;
pub mod signal;
pub mod telemetry;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Adapter(#[from] adapters::AdapterError),

    #[error(transparent)]
    Path(#[from] adapters::PathError),

    #[error(transparent)]
    Log(#[from] adapters::LogReadError),

    #[error(transparent)]
    Telemetry(#[from] telemetry::TelemetryError),

    #[error(transparent)]
    Server(#[from] server::ServerError),

    #[error(transparent)]
    Socket(#[from] daemon::SocketError),

    #[error(transparent)]
    Client(#[from] client::ClientError),

    #[error("cannot prepare the pm3 home '{path}': {reason}")]
    Layout { path: String, reason: String },

    #[error("cannot resolve the apps file '{path}': {reason}")]
    AppsFile { path: String, reason: String },

    #[error("cannot spawn the pm3 daemon: {reason}")]
    DaemonSpawn { reason: String },

    #[error("cannot reach the pm3 daemon on '{path}' within {timeout_ms} ms")]
    DaemonUnready { path: String, timeout_ms: u64 },

    #[error("pm3 daemon refused the request with status {status}: {body}")]
    Refused { status: u16, body: String },
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
#[path = "../test_support/daemon_fixture.rs"]
pub(crate) mod daemon_fixture;
#[cfg(test)]
#[path = "../test_support/config_fixtures.rs"]
pub(crate) mod test_support;
#[cfg(test)]
#[path = "tests/lib_tests.rs"]
mod tests;
