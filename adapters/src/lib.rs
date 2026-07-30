pub mod apps_file;
pub mod config;
pub mod http;
pub mod logs;
pub mod paths;
pub mod persistence;
pub mod presenter;
pub mod process;
pub mod program;
pub mod sandbox;
pub mod service;
pub mod startup;
pub mod state;
pub mod workspace;

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
        AppEntry, AppsFile, AppsFileError, InlineRequest, SERVICE_FILE_SUFFIX, SandboxEntry,
        SpecDefaults, SpecSource, diff_lines, encode_apps_file, inline_apps_file, load_apps_file,
        parse_apps_file, resolve_specs, service_file_of,
    },
    config::{
        AppConfig, ConfigError, LOG_FORMAT_JSON, LOG_FORMAT_PRETTY, LoadedConfig, Pm3Config,
        RestartConfig, SANDBOX_MODE_DANGER_FULL_ACCESS, SANDBOX_MODE_READ_ONLY,
        SANDBOX_MODE_WORKSPACE_WRITE, SandboxConfig, ServiceConfig, TelemetryConfig, check_config,
        load_and_parse_config, load_config_file, parse_config, show_config, validate_config,
        validate_pm3_config, validate_telemetry_config,
    },
    http::{APPS_PATH, HEALTH_OK, HEALTH_PATH, HealthDto, StartRequestDto, router},
    logs::{LogFollower, LogReadError, read_tail, tail_lines},
    paths::{
        CONFIG_FILE, DEFAULT_HOME, PathError, Pm3Paths, default_config_path, expand_home,
        logs_dir_of, resolve_paths,
    },
    persistence::{
        DecodeError, DumpDocument, RuntimeDto, StateDto, YamlDumpStore, decode_state, encode_states,
    },
    presenter::{
        EMPTY_NOTICE, NOTHING_STARTED, already_running_marker, render_describe, render_reply,
        render_started, render_table,
    },
    process::{KILL_PROGRAM, KillSignaler, SystemClock, TokioProcessLauncher},
    program::{
        HOME_PLACEHOLDER, SVC_CWD_NAME, SVC_CWD_PLACEHOLDER, fold_home, fold_svc_cwd,
        program_available, resolve_program,
    },
    sandbox::{SandboxBackend, SandboxCommandWrapper, seatbelt_profile},
    service::{
        CONFIG_FLAG, DAEMON_SUBCOMMAND, NOTHING_INSTALLED, ServiceCommandError, ServiceKind,
        ServiceProgramSet, ServiceStatus, ServiceUnitSpec, install_service, status_report,
        uninstall_service, unit_dir_of,
    },
    startup::log_startup_banner,
    state::{
        DaemonCommand, DaemonError, DaemonFailure, DaemonHandle, DaemonOutcome, DaemonReply,
        DaemonRequest,
    },
    workspace::{expand_svc_cwd, materialise_workspace},
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
#[cfg(test)]
#[path = "../test_support/service_specs.rs"]
pub(crate) mod service_specs;
#[cfg(test)]
#[path = "../test_support/spec_sources.rs"]
pub(crate) mod spec_sources;
