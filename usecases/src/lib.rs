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
pub mod supervisor_ready;
pub mod table;
pub mod timer_state;

mod persist;
mod supervisor_log;

pub use entities::{
    AppSpec, DependencyError, DependencyNode, MemoryVerdict, PolicyError, ProcessIdentity,
    ProcessRuntime, ProcessStatus, ReadScope, ReadyProbe, RestartDecision, RestartPolicy,
    RuntimeError, SandboxMode, SandboxPolicy, SignalNameError, SpecError, VALID_SIGNALS,
    covers_path, decide_memory_verdict, decide_restart, is_name_letter, normalize_root,
    parse_memory_limit, parse_signal_name, topo_sort, validate_app_name, validate_forbidden_roots,
    validate_policy, validate_spec,
};
use thiserror::Error;

pub use self::{
    delete::{DeleteOutcome, delete_app},
    fingerprint::{pid_was_recycled, render_identity},
    handover::{HandoverComparison, ServiceSnapshot, compare_handover, describe_handover},
    log_paths::{LogPaths, LogStream, log_path, log_paths},
    ports::{
        Clock, CommandWrapper, DumpContents, DumpError, DumpStore, ExitOutcome, FingerprintError,
        Fingerprinter, LaunchError, LaunchSpec, LaunchedProcess, Liveness, LogRotateError,
        LogRotator, ProcessLauncher, ProcessProbe, Readiness, ReadyProber, ResourceSample,
        RotatedLog, SandboxError, Scheduler, SignalError, SignalScope, Signaler, SpecResolveError,
        SpecResolver, StrandedProcess, WrappedCommand,
    },
    query::{
        armed_schedule_names, describe_app, identity_token_of, list_apps, owner_of_pid,
        running_pids, schedule_of, unsettled_count,
    },
    record::{ProcessRecord, ProcessView},
    reset::reset_app,
    restart::{RestartOutcome, restart_app},
    resurrect::resurrect,
    selector::AppSelector,
    signal::{SignalOutcome, signal_app},
    start::{
        StartKind, StartOutcome, StartReport, StartSettlement, refused_services, settle_start,
        start_apps,
    },
    stop::{StopOutcome, persist_for_handover, stop_all_apps, stop_app},
    supervise::{ExitAction, handle_child_exit},
    supervision::{
        SupervisionEffect, SupervisionFailure, SupervisionOutcome, SupervisionReply,
        SupervisionRequest,
    },
    supervisor::Supervisor,
    table::ProcessTable,
    timer_state::TimerState,
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
    Dependency(#[from] DependencyError),

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

pub type Result<T> = std::result::Result<T, UsecaseError>;

#[cfg(test)]
#[path = "test_helpers/ports_test_helpers.rs"]
pub(crate) mod ports_test_helpers;
#[cfg(test)]
#[path = "tests/lib_tests.rs"]
mod tests;
