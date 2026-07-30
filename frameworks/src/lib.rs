pub mod cli;
pub mod client;
pub mod commands;
pub mod daemon;
pub mod layout;
pub mod prompt;
pub mod sandbox_probe;
pub mod server;
pub mod service;
pub mod signal;
pub mod svc;
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

    #[error(transparent)]
    Service(#[from] adapters::ServiceCommandError),

    #[error(transparent)]
    Apps(#[from] adapters::AppsFileError),

    #[error(transparent)]
    Signal(#[from] adapters::SignalError),

    #[error("cannot determine the pm3 binary path: {reason}")]
    ServiceProgram { reason: String },

    #[error("cannot resolve the config path '{path}': {reason}")]
    ServiceConfig { path: String, reason: String },

    #[error("cannot locate the service directory: no HOME in the environment")]
    ServiceHome,

    #[error("cannot find '{program}' on PATH")]
    ProgramNotFound { program: String },

    #[error("cannot start: {reason}")]
    InlineUsage { reason: String },

    #[error("cannot write the service file '{path}': {reason}")]
    SvcWrite { path: String, reason: String },

    #[error("cannot overwrite '{path}' without --force:\n{diff}")]
    SvcConflict { path: String, diff: String },

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

    #[error("cannot read the pm3 daemon pid from '{path}'")]
    DaemonPidUnknown { path: String },

    #[error("cannot decode the pm3 daemon reply: {reason}")]
    Undecodable { reason: String },

    #[error(
        "cannot confirm the pm3 daemon (pid {pid}) stopped: '{path}' is still there after {timeout_ms} ms"
    )]
    DaemonLingering {
        pid: u32,
        path: String,
        timeout_ms: u64,
    },
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
