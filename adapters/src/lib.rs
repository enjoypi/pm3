pub mod apps_file;
pub mod config;
pub mod http;
pub mod logs;
pub mod paths;
pub mod persistence;
pub mod presenter;
pub mod process;
pub mod sandbox;
pub mod startup;
pub mod state;

use thiserror::Error;
pub use usecases::{
    AppSelector, AppSpec, Clock, CommandWrapper, DeleteOutcome, DumpError, DumpStore, ExitAction,
    ExitOutcome, LaunchError, LaunchSpec, LaunchedProcess, Ports, ProcessLauncher, ProcessRecord,
    ProcessRuntime, ProcessStatus, ProcessTable, ProcessView, RestartOutcome, SandboxError,
    SandboxMode, SandboxPolicy, SignalError, Signaler, StartOutcome, StopOutcome, UsecaseError,
    WrappedCommand, delete_app, describe_app, handle_child_exit, list_apps, log_paths, restart_app,
    resurrect, start_apps, stop_app, topo_sort,
};

pub use self::{
    apps_file::{
        AppEntry, AppsFile, AppsFileError, SandboxEntry, SpecDefaults, load_apps_file,
        parse_apps_file, resolve_specs,
    },
    config::{
        AppConfig, ConfigError, LOG_FORMAT_JSON, LOG_FORMAT_PRETTY, Pm3Config, RestartConfig,
        SANDBOX_MODE_DANGER_FULL_ACCESS, SANDBOX_MODE_READ_ONLY, SANDBOX_MODE_WORKSPACE_WRITE,
        SandboxConfig, TelemetryConfig, check_config, load_and_parse_config, parse_config,
        show_config, validate_config, validate_pm3_config, validate_telemetry_config,
    },
    http::{APPS_PATH, HEALTH_OK, HEALTH_PATH, HealthDto, StartRequestDto, router},
    logs::{LogFollower, LogReadError, read_tail, tail_lines},
    paths::{PathError, Pm3Paths, expand_home, logs_dir_of, resolve_paths},
    persistence::{
        DecodeError, DumpDocument, RecordDto, RuntimeDto, SandboxDto, YamlDumpStore,
        decode_records, encode_records,
    },
    presenter::{
        EMPTY_NOTICE, NOTHING_STARTED, render_describe, render_reply, render_started, render_table,
    },
    process::{KILL_PROGRAM, KillSignaler, SystemClock, TokioProcessLauncher},
    sandbox::{SandboxBackend, SandboxCommandWrapper, seatbelt_profile},
    startup::log_startup_banner,
    state::{
        DaemonCommand, DaemonError, DaemonFailure, DaemonHandle, DaemonOutcome, DaemonReply,
        DaemonRequest,
    },
};

#[derive(Debug, Error)]
pub enum AdapterError {
    #[error(transparent)]
    Config(#[from] config::ConfigLoadError),

    #[error(transparent)]
    Parse(#[from] ConfigError),
}

pub type Result<T> = std::result::Result<T, AdapterError>;

#[cfg(test)]
#[path = "../test_support/apps_sections.rs"]
pub(crate) mod apps_sections;
#[cfg(test)]
#[path = "../test_support/config_sections.rs"]
pub(crate) mod config_sections;
#[cfg(test)]
#[path = "../test_support/process_records.rs"]
pub(crate) mod process_records;
#[cfg(test)]
#[path = "../test_support/process_views.rs"]
pub(crate) mod process_views;
#[cfg(test)]
#[path = "../test_support/response_body.rs"]
pub(crate) mod response_body;
