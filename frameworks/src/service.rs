use std::path::{Path, PathBuf};

use adapters::{
    AppConfig, Pm3Paths, ServiceKind, ServiceProgramSet, ServiceUnitSpec, install_service,
    load_and_parse_config, status_report, uninstall_service, unit_dir_of,
};

use crate::{
    Error, Result,
    cli::ServiceCommands,
    layout::{ensure_layout, host_home, resolve_cfg_dir, resolve_layout},
};

#[cfg(target_os = "macos")]
const HOST_SERVICE_KIND: ServiceKind = ServiceKind::Launchd;
#[cfg(not(target_os = "macos"))]
const HOST_SERVICE_KIND: ServiceKind = ServiceKind::Systemd;

#[derive(Debug)]
pub struct ServiceContext<'c> {
    pub programs: &'c ServiceProgramSet,
    pub kind: ServiceKind,
    pub home_env: Option<&'c str>,
    pub binary: std::io::Result<PathBuf>,
}

#[derive(Debug)]
pub struct ServiceSession {
    pub paths: Pm3Paths,
    pub cfg_dir: PathBuf,
    pub spec: ServiceUnitSpec,
}

pub async fn run_service(config_path: &str, command: Option<&ServiceCommands>) -> Result<String> {
    let programs = ServiceProgramSet::default();
    let home = host_home();
    let context = ServiceContext {
        programs: &programs,
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
    match command {
        None => Ok(status_report(&session.spec, context.programs).await?),
        Some(ServiceCommands::Install { dry_run }) => install(&session, context, *dry_run).await,
        Some(ServiceCommands::Uninstall { dry_run }) => {
            Ok(uninstall_service(&session.spec, context.programs, *dry_run).await?)
        }
    }
}

pub fn open_service_session(
    config_path: &str,
    context: &ServiceContext<'_>,
) -> Result<ServiceSession> {
    let absolute = canonical_config_path(config_path)?;
    let config = load_and_parse_config(&absolute.to_string_lossy())?;
    let paths = resolve_layout(&config.pm3, context.home_env)?;
    let cfg_dir = resolve_cfg_dir(&config.pm3, context.home_env)?;
    let spec = build_spec(&config, &paths, context, absolute)?;
    Ok(ServiceSession {
        paths,
        cfg_dir,
        spec,
    })
}

async fn install(
    session: &ServiceSession,
    context: &ServiceContext<'_>,
    dry_run: bool,
) -> Result<String> {
    if !dry_run {
        ensure_layout(&session.paths, &session.cfg_dir).await?;
    }
    Ok(install_service(&session.spec, context.programs, dry_run).await?)
}

fn build_spec(
    config: &AppConfig,
    paths: &Pm3Paths,
    context: &ServiceContext<'_>,
    config_path: PathBuf,
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
    let working_directory = paths.root.clone();
    let log_path = paths.daemon_log.clone();
    let home_dir = home.to_string();
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
    })
}

fn canonical_config_path(config_path: &str) -> Result<PathBuf> {
    std::fs::canonicalize(config_path).map_err(|error| Error::ServiceConfig {
        path: config_path.to_string(),
        reason: error.to_string(),
    })
}

#[cfg(test)]
#[path = "tests/service_tests.rs"]
mod tests;
