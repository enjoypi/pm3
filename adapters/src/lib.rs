pub mod apps_file;
pub mod config;
pub mod exit_status;
pub mod http;
pub mod install;
pub mod logs;
pub mod paths;
pub mod persistence;
pub mod presenter;
pub mod private_file;
pub mod process;
pub mod program;
pub mod sandbox;
pub mod schedule;
pub mod service;
pub mod startup;
pub mod state;
pub mod unit;
pub mod workspace;

use thiserror::Error;
pub use usecases::{
    AppSelector, AppSpec, Clock, CommandWrapper, DumpContents, DumpError, DumpStore, ExitOutcome,
    FingerprintError, Fingerprinter, HandoverComparison, LaunchError, LaunchSpec, LaunchedProcess,
    Liveness, LogRotateError, LogRotator, LogStream, Ports, ProcessLauncher, ProcessProbe,
    ProcessRecord, ProcessRuntime, ProcessStatus, ProcessView, ReadScope, Readiness, ReadyProbe,
    ReadyProber, ResourceSample, RotatedLog, SandboxError, SandboxMode, SandboxPolicy, Scheduler,
    ServiceSnapshot, SignalError, SignalScope, Signaler, SpecError, SpecResolveError, StartKind,
    StartOutcome, StartReport, StartSettlement, StrandedProcess, SupervisionEffect,
    SupervisionOutcome, SupervisionReply, SupervisionRequest, Supervisor, WrappedCommand,
    compare_handover, delete_app, describe_app, describe_handover, list_apps, log_path, log_paths,
    settle_start, start_apps, validate_app_name,
};

pub use self::{
    apps_file::{
        AppEntry, AppsFile, AppsFileError, ENV_FILE_SUFFIX, EnvFileError, InlineRequest,
        ReadyProbeEntry, SERVICE_FILE_SUFFIX, SandboxEntry, SpecDefaults, SpecSource, diff_lines,
        encode_service_file, env_file_of, fold_entry, inline_entry, load_apps_file, load_env_file,
        load_service_file, parse_apps_file, parse_env_file, parse_service_file, resolve_checked,
        service_file_of,
    },
    config::{
        AppConfig, ConfigError, LOG_FORMAT_JSON, LOG_FORMAT_PRETTY, LoadedConfig, Pm3Config,
        RESTART_CONDITION_ALWAYS, RESTART_CONDITION_ON_FAILURE, RestartConfig, STOP_SIGNAL_TERM,
        SandboxConfig, ServiceConfig, TelemetryConfig, check_config, load_and_parse_config,
        load_config_file, parse_config, show_config, validate_config, validate_pm3_config,
        validate_telemetry_config,
    },
    exit_status::{UNKNOWN_EXIT_CODE, describe_refusal, exit_code_of},
    http::{
        APPS_PATH, HEALTH_OK, HEALTH_PATH, HealthDto, ProcessViewDto, REQUEST_ID_HEADER,
        ReplyDecodeError, ReplyDto, SERVICES_STOP_ALL_PATH, StartRequestDto, app_action_path,
        app_path, decode_reply, encode_signal_request, encode_start_request, router,
    },
    install::{
        InstallError, back_up, backup_name, backup_root, binary_version, destination_of,
        parse_version_output, replace_binary,
    },
    logs::{
        CopyTruncateRotator, LogClearError, LogFollower, LogReadError, clear_log, read_tail,
        tail_lines,
    },
    paths::{
        CONFIG_FILE, DEFAULT_HOME, PathError, Pm3Paths, default_config_path, expand_home,
        resolve_paths,
    },
    persistence::{
        DecodeError, DumpDocument, RuntimeDto, StateDto, YamlDumpStore, decode_state,
        dump_snapshot, encode_states,
    },
    presenter::{
        DAEMON_NOT_RUNNING, EMPTY_NOTICE, NOTHING_STARTED, affected_service, already_running_names,
        render_daemon_gone, render_daemon_stopped, render_describe, render_json_list,
        render_json_one, render_reply, render_started, render_table, unsaved_reason,
    },
    private_file::{OWNER_ONLY_FILE, append_private, append_private_blocking, write_private},
    process::{
        AdoptedWatch, HostReadyProber, KILL_PROGRAM, KillSignaler, PS_PROGRAM, PollCadence,
        PsProcessProbe, Sha256Fingerprinter, SystemClock, TokioProcessLauncher, wait_for_exit,
        wait_until_released,
    },
    program::{
        HOME_PLACEHOLDER, SERVICE_CWD_NAME, SERVICE_CWD_PLACEHOLDER, fold_home, fold_service_cwd,
        program_available, resolve_program,
    },
    sandbox::{
        HostSandbox, SandboxBackend, SandboxCommandWrapper, SandboxProgramSet, seatbelt_profile,
    },
    schedule::{CronError, CronScheduler, ExpandError, expand_random, validate_cron},
    service::{
        InlineStart, PreparedService, Reconciled, ServiceContext, ServiceError, ServiceUndo,
        SplitApps, forget, prepare_inline, reconcile, split_apps_file,
    },
    startup::log_startup_banner,
    state::{DaemonCommand, DaemonError, DaemonHandle},
    unit::{
        CONFIG_FLAG, DAEMON_SUBCOMMAND, NOTHING_INSTALLED, UnitCommandError, UnitKind,
        UnitProgramSet, UnitSpec, UnitStatus, hand_back_to_manager, install_unit, pm3_variables,
        query_status, query_supervised_pid, runtime_dir_of, status_report, uninstall_unit,
        unit_dir_of, write_targets,
    },
    workspace::{expand_service_cwd, materialise_workspace},
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
#[path = "../test_support/service_fixtures.rs"]
pub(crate) mod service_fixtures;
#[cfg(test)]
#[path = "../test_support/spec_sources.rs"]
pub(crate) mod spec_sources;
#[cfg(test)]
#[path = "../test_support/unit_specs.rs"]
pub(crate) mod unit_specs;
