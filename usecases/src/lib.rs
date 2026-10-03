pub mod delete;
pub mod fingerprint;
pub mod handover;
pub mod log_paths;
pub mod ports;
pub mod query;
pub mod record;
pub mod reset;
pub mod restart;
pub mod resurrect;
pub mod selector;
pub mod signal;
pub mod start;
pub mod stop;
pub mod supervise;
pub mod supervision;
pub mod supervisor;
pub mod supervisor_handlers;
pub mod supervisor_liveness;
pub mod supervisor_ready;
pub mod table;
pub mod timer_state;

mod persist;
mod supervisor_log;

pub use entities::{
    AppSpec, EnvOrigin, EnvScope, EnvValue, PolicyError, ProcessIdentity, ProcessRuntime,
    ProcessStatus, ReadScope, ReadyProbe, RestartPolicy, RuntimeError, SandboxMode, SandboxPolicy,
    SignalNameError, SpecError, VALID_SIGNALS, covers_path, is_absolute_path, is_name_letter,
    mask_secret, merge_environment, normalize_root, parse_memory_limit, root_is_forbidden,
    validate_app_name, validate_forbidden_roots, validate_policy, validate_spec,
};
use thiserror::Error;

pub use self::{
    delete::delete_app,
    handover::{ServiceSnapshot, compare_handover, describe_handover},
    log_paths::{LogStream, log_path},
    ports::{
        Clock, CommandWrapper, DumpContents, DumpError, DumpStore, ExitOutcome, FingerprintError,
        Fingerprinter, LaunchError, LaunchSpec, LaunchedProcess, Liveness, LogRotateError,
        LogRotator, ProcessLauncher, ProcessProbe, Readiness, ReadyProber, ResourceSample,
        RotatedLog, SandboxError, Scheduler, SignalError, SignalScope, Signaler, SpecResolveError,
        SpecResolver, StrandedProcess, WrappedCommand,
    },
    query::{describe_app, list_apps},
    record::{EnvDisplay, ProcessRecord, ProcessView},
    selector::AppSelector,
    signal::signal_app,
    start::{StartKind, StartOutcome, StartReport, StartSettlement, settle_start, start_apps},
    supervision::{
        SupervisionEffect, SupervisionFailure, SupervisionOutcome, SupervisionReply,
        SupervisionRequest,
    },
    supervisor::Supervisor,
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
    + LogRotator
    + ReadyProber
{
}

#[derive(Debug, Error)]
pub enum UsecaseError {
    #[error(transparent)]
    Spec(#[from] SpecError),

    #[error(transparent)]
    Dependency(#[from] entities::DependencyError),

    #[error(transparent)]
    Policy(#[from] PolicyError),

    #[error(transparent)]
    Launch(#[from] LaunchError),

    #[error(transparent)]
    Signal(#[from] SignalError),

    #[error(transparent)]
    InvalidSignal(#[from] SignalNameError),

    #[error(transparent)]
    Sandbox(#[from] SandboxError),

    #[error(transparent)]
    Dump(#[from] DumpError),

    #[error(transparent)]
    Fingerprint(#[from] FingerprintError),

    #[error("cannot find app '{0}'")]
    NotFound(String),

    #[error("cannot signal '{0}': it is not running")]
    NotRunning(String),

    #[error("cannot delete app '{name}': {} still depends on it", .dependents.join(", "))]
    StillDependedOn {
        name: String,
        dependents: Vec<String>,
    },
}

impl UsecaseError {
    #[must_use]
    pub const fn blocks_takeover(&self) -> bool {
        matches!(self, Self::Dump(DumpError::Unreadable { .. }))
    }
}

pub type Result<T> = std::result::Result<T, UsecaseError>;

#[cfg(test)]
#[path = "test_helpers/ports_fixture_tests.rs"]
pub(crate) mod ports_test_helpers;
#[cfg(test)]
#[path = "tests/lib_tests.rs"]
mod tests;

#[cfg(test)]
#[ctor::ctor(unsafe)]
fn trace_every_callsite() {
    test_trace::install();
}
