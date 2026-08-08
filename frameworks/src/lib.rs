pub mod cli;
pub mod client;
pub mod commands;
pub mod daemon;
pub mod install;
pub mod layout;
pub mod prompt;
pub mod sandbox_probe;
pub mod server;
pub mod service;
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

    #[error(transparent)]
    Service(#[from] adapters::UnitCommandError),

    #[error(transparent)]
    ServiceFile(#[from] adapters::ServiceError),

    #[error(transparent)]
    Install(#[from] adapters::InstallError),

    #[error(transparent)]
    Dump(#[from] adapters::DumpError),

    #[error(
        "the pm3 service did not come under the service manager's supervision within {timeout_ms} ms; the previous install is backed up in '{backup}' — restore the binary, unit and config from there, then run `pm3 service install --force`"
    )]
    InstallTakeover { timeout_ms: u64, backup: String },

    #[error("not every managed service came back after the install:\n{report}")]
    InstallLost { report: String },

    #[error(transparent)]
    Signal(#[from] adapters::SignalError),

    #[error(transparent)]
    SignalRegister(#[from] signal::SignalRegisterError),

    #[error(transparent)]
    Cron(#[from] adapters::CronError),

    #[error(transparent)]
    Spec(#[from] adapters::SpecError),

    #[error("cannot determine the pm3 binary path: {reason}")]
    ServiceProgram { reason: String },

    #[error("cannot resolve the config path '{path}': {reason}")]
    ServiceConfig { path: String, reason: String },

    #[error("cannot locate the service directory: no HOME in the environment")]
    ServiceHome,

    #[error("cannot start: {reason}")]
    InlineUsage { reason: String },

    #[error("cannot prepare the pm3 home '{path}': {reason}")]
    Layout { path: String, reason: String },

    #[error("cannot resolve the apps file '{path}': {reason}")]
    AppsFile { path: String, reason: String },

    #[error("cannot spawn the pm3 daemon: {reason}")]
    DaemonSpawn { reason: String },

    #[error("cannot reach the pm3 daemon on '{path}' within {timeout_ms} ms")]
    DaemonUnready { path: String, timeout_ms: u64 },

    #[error("cannot complete the request: the pm3 daemon answered status {status}: {body}")]
    Refused { status: u16, body: String },

    #[error("cannot start {refused}:\n{report}")]
    PartialStart { refused: String, report: String },

    #[error("cannot record what pm3 just started:\n{report}")]
    UnsavedStart { report: String },

    #[error("cannot read the pm3 daemon pid from '{path}'")]
    DaemonPidUnknown { path: String },

    #[error(transparent)]
    Undecodable(#[from] adapters::ReplyDecodeError),

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
