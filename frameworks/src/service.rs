use std::path::{Path, PathBuf};

use adapters::{
    AppConfig, Pm3Paths, UnitKind, UnitProgramSet, UnitSpec, install_unit, load_config_file,
    pm3_variables, status_report, uninstall_unit, unit_dir_of,
};

use crate::{
    Error, Result,
    layout::{
        Pm3Places, canonicalize, ensure_layout, host_home, host_pm3_env, host_runtime_dir,
        host_uid, resolve_places,
    },
    telemetry::init_cli_telemetry,
};

const OWNER_ONLY_UMASK: u32 = 0o077;
const ROOT_VARIABLES: [&str; 4] = [
    "PM3_CONFIG_DIR",
    "PM3_STATE_DIR",
    "PM3_RUNTIME_DIR",
    "PM3_DATA_DIR",
];

#[cfg(target_os = "macos")]
pub(crate) const HOST_SERVICE_KIND: UnitKind = UnitKind::Launchd;
#[cfg(windows)]
pub(crate) const HOST_SERVICE_KIND: UnitKind = UnitKind::WinSchtasks;
#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) const HOST_SERVICE_KIND: UnitKind = UnitKind::Systemd;

#[derive(Debug)]
pub struct ServiceContext<'c> {
    pub programs: Option<&'c UnitProgramSet>,
    pub kind: UnitKind,
    pub home_env: Option<&'c str>,
    pub pm3_env: Vec<(String, String)>,
    pub runtime_dir: Option<String>,
    pub uid: Option<u32>,
    pub binary: std::io::Result<PathBuf>,
}

#[derive(Debug)]
pub struct ServiceSession {
    pub paths: Pm3Paths,
    pub cfg_dir: PathBuf,
    pub spec: UnitSpec,
    pub source: String,
    pub programs: UnitProgramSet,
    pub command_timeout_ms: u64,
    pub start_timeout_ms: u64,
    pub request_timeout_ms: u64,
    pub daemon_poll_interval_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServiceAction {
    Status,
    Install { dry_run: bool, force: bool },
    Uninstall { dry_run: bool },
}

pub async fn run_service(config_path: &str, action: &ServiceAction) -> Result<String> {
    let home = host_home();
    let context = ServiceContext {
        programs: None,
        kind: HOST_SERVICE_KIND,
        home_env: home.as_deref(),
        pm3_env: host_pm3_env(),
        runtime_dir: host_runtime_dir(home.as_deref()),
        uid: host_uid(home.as_deref()),
        binary: std::env::current_exe(),
    };
    dispatch_service(config_path, action, &context).await
}

pub async fn dispatch_service(
    config_path: &str,
    action: &ServiceAction,
    context: &ServiceContext<'_>,
) -> Result<String> {
    let session = open_service_session(config_path, context)?;
    let programs = context.programs.unwrap_or(&session.programs);
    let timeout_ms = session.command_timeout_ms;
    match action {
        ServiceAction::Status => Ok(status_report(&session.spec, programs, timeout_ms).await?),
        ServiceAction::Install { dry_run, force } => {
            install(&session, programs, *dry_run, *force).await
        }
        ServiceAction::Uninstall { dry_run } => {
            Ok(uninstall_unit(&session.spec, programs, *dry_run, timeout_ms).await?)
        }
    }
}

pub fn open_service_session(
    config_path: &str,
    context: &ServiceContext<'_>,
) -> Result<ServiceSession> {
    let absolute = canonical_config_path(config_path)?;
    let loaded = load_config_file(&absolute.to_string_lossy())?;
    init_cli_telemetry(&loaded.config.telemetry);
    let Pm3Places { paths, cfg_dir } = resolve_places(&loaded.config.pm3, context.home_env)?;
    let spec = build_spec(&loaded.config, &paths, context)?;
    let programs = UnitProgramSet::from_config(
        &loaded.config.pm3.service,
        context.runtime_dir.as_deref(),
        context.uid,
    );
    let command_timeout_ms = loaded.config.pm3.command_timeout_ms;
    Ok(ServiceSession {
        paths,
        cfg_dir,
        spec,
        source: loaded.source,
        programs,
        command_timeout_ms,
        start_timeout_ms: loaded.config.pm3.start_timeout_ms,
        request_timeout_ms: loaded.config.pm3.request_timeout_ms,
        daemon_poll_interval_ms: loaded.config.pm3.daemon_poll_interval_ms,
    })
}

async fn install(
    session: &ServiceSession,
    programs: &UnitProgramSet,
    dry_run: bool,
    force: bool,
) -> Result<String> {
    if !dry_run {
        ensure_layout(&session.paths, &session.cfg_dir).await?;
        adapters::reconcile(&session.spec.config_path, &session.source, force).await?;
    }
    Ok(install_unit(
        &session.spec,
        programs,
        &session.source,
        dry_run,
        session.command_timeout_ms,
    )
    .await?)
}

fn resolved_roots_env(inherited: &[(String, String)], paths: &Pm3Paths) -> Vec<(String, String)> {
    let mut kept: Vec<(String, String)> = inherited
        .iter()
        .filter(|(name, _value)| !ROOT_VARIABLES.contains(&name.as_str()))
        .cloned()
        .collect();
    kept.extend(root_variables(paths));
    kept
}

fn root_variables(paths: &Pm3Paths) -> Vec<(String, String)> {
    [
        (ROOT_VARIABLES[0], &paths.roots.config),
        (ROOT_VARIABLES[1], &paths.roots.state),
        (ROOT_VARIABLES[2], &paths.roots.runtime),
        (ROOT_VARIABLES[3], &paths.roots.data),
    ]
    .into_iter()
    .map(|(name, root)| (name.to_string(), root.to_string_lossy().into_owned()))
    .collect()
}
fn build_spec(
    config: &AppConfig,
    paths: &Pm3Paths,
    context: &ServiceContext<'_>,
) -> Result<UnitSpec> {
    let home = context.home_env.ok_or(Error::ServiceHome)?;
    let program = context
        .binary
        .as_ref()
        .map_err(|error| Error::ServiceProgram {
            reason: error.to_string(),
        })?
        .clone();
    let kind = context.kind;
    let label = config.pm3.service.label.clone();
    let search_path = config.pm3.search_path.clone();
    let unit_dir = unit_dir_of(kind, Path::new(home));
    let config_path = paths.config_file.clone();
    let working_directory = paths.root.clone();
    let log_path = paths.daemon_log.clone();
    let home_dir = home.to_string();
    let restart_delay_secs = config.pm3.service.restart_delay_secs;
    let restart_condition = config.pm3.service.restart_condition.clone();
    Ok(UnitSpec {
        kind,
        label,
        unit_dir,
        program,
        config_path,
        working_directory,
        log_path,
        search_path,
        home: home_dir,
        pm3_env: pm3_variables(resolved_roots_env(&context.pm3_env, paths)),
        restart_delay_secs,
        restart_condition,
        umask: OWNER_ONLY_UMASK,
        max_tasks: config.pm3.service.max_tasks,
        cpu_quota_percent: config.pm3.service.cpu_quota_percent,
        wait_for_network: config.pm3.service.wait_for_network,
    })
}

fn canonical_config_path(config_path: &str) -> Result<PathBuf> {
    canonicalize(config_path, |reason| Error::ServiceConfig {
        path: config_path.to_string(),
        reason,
    })
}

#[cfg(test)]
#[path = "tests/service_tests.rs"]
mod tests;
