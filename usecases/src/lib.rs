pub mod delete;
pub mod fingerprint;
pub mod log_paths;
pub mod ports;
pub mod query;
pub mod record;
pub mod restart;
pub mod resurrect;
pub mod selector;
pub mod start;
pub mod stop;
pub mod supervise;
pub mod table;

mod persist;

pub use entities::{
    AppSpec, DependencyError, DependencyNode, PolicyError, ProcessIdentity, ProcessRuntime,
    ProcessStatus, RestartDecision, RestartPolicy, SandboxMode, SandboxPolicy, SpecError,
    decide_restart, topo_sort, validate_app_name, validate_spec,
};
use thiserror::Error;

pub use self::{
    delete::{DeleteOutcome, delete_app},
    fingerprint::render_identity,
    log_paths::{LogPaths, log_paths},
    ports::{
        Clock, CommandWrapper, DumpError, DumpStore, ExitOutcome, FingerprintError, Fingerprinter,
        LaunchError, LaunchSpec, LaunchedProcess, Liveness, ProcessLauncher, ProcessProbe,
        SandboxError, Scheduler, SignalError, Signaler, WrappedCommand,
    },
    query::{describe_app, list_apps},
    record::{ProcessRecord, ProcessView},
    restart::{RestartOutcome, restart_app},
    resurrect::resurrect,
    selector::AppSelector,
    start::{StartKind, StartOutcome, StartReport, start_apps},
    stop::{StopOutcome, persist_for_handover, stop_all_apps, stop_app},
    supervise::{ExitAction, handle_child_exit},
    table::ProcessTable,
};

pub trait Ports:
    ProcessLauncher
    + Signaler
    + CommandWrapper
    + DumpStore
    + Clock
    + ProcessProbe
    + Fingerprinter
    + Scheduler
{
}

#[derive(Debug, Error)]
pub enum UsecaseError {
    #[error(transparent)]
    Spec(#[from] SpecError),

    #[error(transparent)]
    Dependency(#[from] DependencyError),

    #[error(transparent)]
    Policy(#[from] PolicyError),

    #[error(transparent)]
    Launch(#[from] LaunchError),

    #[error(transparent)]
    Signal(#[from] SignalError),

    #[error(transparent)]
    Sandbox(#[from] SandboxError),

    #[error(transparent)]
    Dump(#[from] DumpError),

    #[error(transparent)]
    Fingerprint(#[from] FingerprintError),

    #[error("cannot find app '{0}'")]
    NotFound(String),

    #[error("cannot delete app '{name}': {} still depends on it", .dependents.join(", "))]
    StillDependedOn {
        name: String,
        dependents: Vec<String>,
    },
}

pub type Result<T> = std::result::Result<T, UsecaseError>;

#[cfg(test)]
#[path = "test_helpers/ports_test_helpers.rs"]
pub(crate) mod ports_test_helpers;
#[cfg(test)]
#[path = "tests/lib_tests.rs"]
mod tests;
