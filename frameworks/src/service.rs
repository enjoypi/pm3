use std::path::{Path, PathBuf};

use adapters::{
    AppConfig, Pm3Paths, ServiceKind, ServiceProgramSet, ServiceUnitSpec, install_service,
    load_config_file, status_report, uninstall_service, unit_dir_of,
};

use crate::{
    Error, Result,
    cli::ServiceCommands,
    layout::{canonicalize, ensure_layout, host_home, resolve_cfg_dir, resolve_layout},
    svc,
};

#[cfg(target_os = "macos")]
const HOST_SERVICE_KIND: ServiceKind = ServiceKind::Launchd;
#[cfg(not(target_os = "macos"))]
const HOST_SERVICE_KIND: ServiceKind = ServiceKind::Systemd;

#[derive(Debug)]
pub struct ServiceContext<'c> {
    pub programs: Option<&'c ServiceProgramSet>,
    pub kind: ServiceKind,
    pub home_env: Option<&'c str>,
    pub binary: std::io::Result<PathBuf>,
}

#[derive(Debug)]
pub struct ServiceSession {
    pub paths: Pm3Paths,
    pub cfg_dir: PathBuf,
    pub spec: ServiceUnitSpec,
    pub source: String,
    pub programs: ServiceProgramSet,
    pub command_timeout_ms: u64,
}

pub async fn run_service(config_path: &str, command: Option<&ServiceCommands>) -> Result<String> {
    let home = host_home();
    let context = ServiceContext {
        programs: None,
        kind: HOST_SERVICE_KIND,
        home_env: home.as_deref(),
        binary: std::env::current_exe(),
    };
    dispatch_service(config_path, command, &context).await
}

pub async fn dispatch_service(
    config_path: &str,
    command: Option<&ServiceCommands>,
    context: &ServiceContext<'_>,
) -> Result<String> {
    let session = open_service_session(config_path, context)?;
    let programs = context.programs.unwrap_or(&session.programs);
    let timeout_ms = session.command_timeout_ms;
    match command {
        None => Ok(status_report(&session.spec, programs, timeout_ms).await?),
        Some(ServiceCommands::Install { dry_run, force }) => {
            install(&session, programs, *dry_run, *force).await
        }
        Some(ServiceCommands::Uninstall { dry_run }) => {
            Ok(uninstall_service(&session.spec, programs, *dry_run, timeout_ms).await?)
        }
    }
}

pub fn open_service_session(
    config_path: &str,
    context: &ServiceContext<'_>,
) -> Result<ServiceSession> {
    let absolute = canonical_config_path(config_path)?;
    let loaded = load_config_file(&absolute.to_string_lossy())?;
    let paths = resolve_layout(&loaded.config.pm3, context.home_env)?;
    let cfg_dir = resolve_cfg_dir(&loaded.config.pm3, context.home_env)?;
    let spec = build_spec(&loaded.config, &paths, context)?;
    let programs = ServiceProgramSet::from_config(&loaded.config.pm3.service);
    let command_timeout_ms = loaded.config.pm3.command_timeout_ms;
    Ok(ServiceSession {
        paths,
        cfg_dir,
        spec,
        source: loaded.source,
        programs,
        command_timeout_ms,
    })
}

async fn install(
    session: &ServiceSession,
    programs: &ServiceProgramSet,
    dry_run: bool,
    force: bool,
) -> Result<String> {
    if !dry_run {
        ensure_layout(&session.paths, &session.cfg_dir).await?;
        svc::reconcile(&session.spec.config_path, &session.source, force).await?;
    }
    Ok(install_service(
        &session.spec,
        programs,
        &session.source,
        dry_run,
        session.command_timeout_ms,
    )
    .await?)
}

fn build_spec(
    config: &AppConfig,
    paths: &Pm3Paths,
    context: &ServiceContext<'_>,
) -> Result<ServiceUnitSpec> {
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
    Ok(ServiceUnitSpec {
        kind,
        label,
        unit_dir,
        program,
        config_path,
        working_directory,
        log_path,
        search_path,
        home: home_dir,
        restart_delay_secs,
        restart_condition,
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
