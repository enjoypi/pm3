pub mod apps_file;
pub mod config;
pub mod exit_status;
pub mod fs_util;
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
    AppSelector, AppSpec, Clock, CommandWrapper, DumpContents, DumpError, DumpStore, EnvOrigin,
    ExitOutcome, FingerprintError, Fingerprinter, LaunchError, LaunchSpec, LaunchedProcess,
    Liveness, LogRotateError, LogRotator, LogStream, Ports, ProcessLauncher, ProcessProbe,
    ProcessRecord, ProcessRuntime, ProcessStatus, ProcessView, ReadScope, Readiness, ReadyProbe,
    ReadyProber, ResourceSample, RotatedLog, SandboxError, SandboxMode, SandboxPolicy, Scheduler,
    SignalError, SignalScope, Signaler, SpecError, SpecResolveError, StartKind, StartOutcome,
    StartSettlement, SupervisionEffect, SupervisionOutcome, SupervisionReply, SupervisionRequest,
    Supervisor, WrappedCommand, compare_handover, describe_handover, log_path, settle_start,
    validate_app_name,
};

pub use self::{
    apps_file::{
        AppsFile, AppsFileError, ENC_FILE_SUFFIX, ENV_FILE_SUFFIX, GLOBAL_ENV_STEM, InlineStart,
        SERVICE_FILE_SUFFIX, SpecDefaults, SpecRoots, SpecSource, decryptor_env, enc_file_of,
        encode_service_file, env_file_of, fold_entry, inline_entry, load_apps_file,
        load_global_env, service_file_of, warn_misplaced_global_env,
    },
    config::{
        AppConfig, ConfigError, LOG_FORMAT_PRETTY, Pm3Config, RestartConfig, STOP_SIGNAL_TERM,
        SandboxConfig, ServiceConfig, TelemetryConfig, check_config, load_and_parse_config,
        load_config_file, parse_config, show_config,
    },
    exit_status::{describe_refusal, exit_code_of},
    fs_util::remove_if_present,
    http::{
        APPS_PATH, HEALTH_OK, HEALTH_PATH, ProcessViewDto, REQUEST_ID_HEADER, RESET_ACTION,
        RESTART_ACTION, ReplyDecodeError, ReplyDto, SERVICES_STOP_ALL_PATH, SIGNAL_ACTION,
        STOP_ACTION, app_action_path, app_path, decode_reply, encode_signal_request,
        encode_start_request, router,
    },
    install::{
        InstallError, back_up, backup_name, backup_root, binary_matches, binary_version,
        destination_of, replace_binary,
    },
    logs::{CopyTruncateRotator, LogClearError, LogFollower, LogReadError, clear_log, read_tail},
    paths::{
        CONFIG_FILE, PathError, Pm3Paths, Pm3Roots, RUNTIME_DIR_VARIABLE, RuntimeSources,
        check_socket_length, default_config_path, expand_home, portable_path, portable_real_path,
        resolve_config_root, resolve_data_root, resolve_paths, resolve_runtime_root,
        resolve_state_root,
    },
    persistence::{YamlDumpStore, dump_snapshot},
    presenter::{
        DAEMON_NOT_RUNNING, EMPTY_NOTICE, Listing, affected_service, already_running_names,
        render_daemon_gone, render_daemon_stopped, render_json_list, render_json_one, render_reply,
        render_table, unsaved_reason,
    },
    private_file::{
        OWNER_ONLY_DIR, OWNER_ONLY_FILE, SWEEP_PROOF_FILE, append_private_blocking, write_private,
        write_sweep_proof,
    },
    process::{
        AdoptedWatch, HostProcessProbe, HostReadyProber, KillSignaler, PollCadence,
        Sha256Fingerprinter, SystemClock, TokioProcessLauncher, wait_for_exit, wait_until_released,
    },
    sandbox::{HostSandbox, SandboxBackend, SandboxCommandWrapper, SandboxProgramSet},
    schedule::{CronError, CronScheduler, validate_cron},
    service::{
        Reconciled, ServiceContext, ServiceError, ServiceUndo, forget, prepare_inline, reconcile,
        split_apps_file,
    },
    startup::log_startup_banner,
    state::{DaemonCommand, DaemonHandle},
    unit::{
        CONFIG_FLAG, DAEMON_SUBCOMMAND, UnitCommandError, UnitKind, UnitProgramSet, UnitSpec,
        UnitStatus, hand_back_to_manager, install_unit, pm3_variables, query_status,
        query_supervised_pid, runtime_dir_of, status_report, uninstall_unit, unit_dir_of,
        write_targets,
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
#[path = "../test_support/apps_sections_fixture_tests.rs"]
pub(crate) mod apps_sections;
#[cfg(test)]
#[path = "../test_support/config_sections_fixture_tests.rs"]
pub(crate) mod config_sections;
#[cfg(test)]
#[path = "../test_support/platform_fixture_tests.rs"]
pub(crate) mod platform;
#[cfg(test)]
#[path = "../test_support/process_records_fixture_tests.rs"]
pub(crate) mod process_records;
#[cfg(test)]
#[path = "../test_support/process_views_fixture_tests.rs"]
pub(crate) mod process_views;
#[cfg(test)]
#[path = "../test_support/response_body_fixture_tests.rs"]
pub(crate) mod response_body;
#[cfg(test)]
#[path = "../test_support/service_fixture_tests.rs"]
pub(crate) mod service_fixtures;
#[cfg(test)]
#[path = "../test_support/spec_sources_fixture_tests.rs"]
pub(crate) mod spec_sources;
#[cfg(test)]
#[path = "../test_support/unit_specs_fixture_tests.rs"]
pub(crate) mod unit_specs;
