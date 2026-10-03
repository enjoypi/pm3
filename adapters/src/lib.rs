mod apps_file;
mod config;
mod exit_status;
mod fs_util;
mod http;
mod install;
mod logs;
mod paths;
mod persistence;
mod presenter;
mod private_file;
mod process;
mod program;
mod sandbox;
mod schedule;
mod service;
mod startup;
mod state;
mod unit;
mod workspace;

use thiserror::Error;
pub use usecases::{
    AppSelector, AppSpec, Clock, CommandWrapper, DumpContents, DumpError, DumpStore, EnvOrigin,
    ExitOutcome, FingerprintError, Fingerprinter, LaunchError, LaunchSpec, LaunchedProcess,
    Liveness, LogRotateError, LogRotator, LogStream, Ports, ProcessLauncher, ProcessProbe,
    ProcessRecord, ProcessRuntime, ProcessStatus, ProcessView, ReadScope, Readiness, ReadyProbe,
    ReadyProber, ResourceSample, RotatedLog, SandboxError, SandboxMode, SandboxPolicy, Scheduler,
    SignalError, SignalScope, Signaler, SpecError, StartKind, StartOutcome, StartSettlement,
    SupervisionEffect, SupervisionOutcome, SupervisionReply, SupervisionRequest, Supervisor,
    WrappedCommand, compare_handover, describe_handover, log_path, settle_start, validate_app_name,
};

pub use self::{
    apps_file::{
        AppsFileError, ENV_FILE_SUFFIX, GLOBAL_ENV_STEM, InlineStart, SERVICE_FILE_SUFFIX,
        SpecSource, decryptor_env, load_global_env, service_file_of, warn_misplaced_global_env,
    },
    config::{
        AppConfig, DEFAULT_CONFIG, LOG_FORMAT_PRETTY, Pm3Config, RestartConfig, STOP_SIGNAL_TERM,
        SandboxConfig, ServiceConfig, TelemetryConfig, check_config, load_and_parse_config,
        load_config_file, show_config,
    },
    fs_util::remove_if_present,
    http::{
        APPS_PATH, HEALTH_PATH, REQUEST_ID_HEADER, RESET_ACTION, RESTART_ACTION, ReplyDecodeError,
        ReplyDto, SERVICES_STOP_ALL_PATH, SIGNAL_ACTION, STOP_ACTION, app_action_path, app_path,
        decode_reply, encode_signal_request, encode_start_request, router,
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
        DAEMON_NOT_RUNNING, Listing, render_daemon_gone, render_daemon_stopped, render_json_list,
        render_json_one, render_table,
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
    Parse(#[from] config::ConfigError),
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

#[cfg(test)]
#[ctor::ctor(unsafe)]
fn trace_every_callsite() {
    test_trace::install();
}
