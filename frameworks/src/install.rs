use std::{
    io,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use adapters::{
    UnitKind, UnitProgramSet, UnitStatus, back_up, backup_name, backup_root, binary_version,
    compare_handover, describe_handover, destination_of, dump_snapshot, hand_back_to_manager,
    query_status, query_supervised_pid, replace_binary, write_targets,
};

use crate::{
    Error, Result,
    cli::ServiceCommands,
    client::UdsClient,
    commands,
    layout::{
        host_home, host_install_backups, host_install_destination, host_pm3_env, host_runtime_dir,
        host_uid, read_pid_file,
    },
    service::{
        HOST_SERVICE_KIND, ServiceContext, ServiceSession, dispatch_service, open_service_session,
    },
};

#[derive(Debug)]
pub struct InstallContext {
    pub home_env: Option<String>,
    pub destination_env: Option<String>,
    pub backups_env: Option<String>,
    pub pm3_env: Vec<(String, String)>,
    pub runtime_dir: Option<String>,
    pub uid: Option<u32>,
    pub current_exe: io::Result<PathBuf>,
    pub kind: UnitKind,
    pub programs: Option<UnitProgramSet>,
}

pub async fn run(config_path: &str, source: Option<PathBuf>) -> Result<()> {
    let context = InstallContext {
        home_env: host_home(),
        destination_env: host_install_destination(),
        backups_env: host_install_backups(),
        pm3_env: host_pm3_env(),
        runtime_dir: host_runtime_dir(),
        uid: host_uid(),
        current_exe: std::env::current_exe(),
        kind: HOST_SERVICE_KIND,
        programs: None,
    };
    run_install(config_path, source, &context, &emit).await
}

pub async fn run_install(
    config_path: &str,
    source: Option<PathBuf>,
    context: &InstallContext,
    emit: &(dyn Fn(&str) + Send + Sync),
) -> Result<()> {
    let source = resolve_source(source, context)?;
    let destination = destination_of(
        context.destination_env.as_deref(),
        context.home_env.as_deref(),
    )?;
    let service_context = service_context(context, &destination);
    let session = open_service_session(config_path, &service_context)?;
    let programs = context.programs.as_ref().unwrap_or(&session.programs);

    let before = dump_snapshot(&session.paths.dump_file).await?;
    let root = backup_root(context.backups_env.as_deref(), &session.paths.root);
    let stamp = backup_name(binary_version(&destination).await.as_deref());
    let targets = write_targets(&session.spec);
    let backup = back_up(std::slice::from_ref(&destination), &root, &stamp).await?;
    replace_binary(&source, &destination).await?;
    back_up(&targets, &root, &stamp).await?;
    emit(&format!("backed up {}", backup.display()));
    log_step("swap", &source, &destination);

    emit(
        &dispatch_service(
            config_path,
            Some(&ServiceCommands::Uninstall { dry_run: false }),
            &service_context,
        )
        .await?,
    );
    emit(&commands::kill_daemon(config_path, false).await?);
    emit(
        &dispatch_service(
            config_path,
            Some(&ServiceCommands::Install {
                dry_run: false,
                force: true,
            }),
            &service_context,
        )
        .await?,
    );

    wait_for_takeover(&session, programs, &backup, emit).await?;
    log_step("takeover", &session.spec.unit_path(), &backup);

    let after = dump_snapshot(&session.paths.dump_file).await?;
    let comparison = compare_handover(&before, &after);
    let description = describe_handover(&comparison);
    emit(&description);
    if comparison.lost.is_empty() {
        return Ok(());
    }
    Err(Error::InstallLost {
        report: description,
    })
}

fn resolve_source(source: Option<PathBuf>, context: &InstallContext) -> Result<PathBuf> {
    source.map_or_else(
        || {
            context
                .current_exe
                .as_ref()
                .map_err(|error| Error::ServiceProgram {
                    reason: error.to_string(),
                })
                .cloned()
        },
        Ok,
    )
}

fn service_context<'c>(context: &'c InstallContext, destination: &Path) -> ServiceContext<'c> {
    ServiceContext {
        programs: context.programs.as_ref(),
        kind: context.kind,
        home_env: context.home_env.as_deref(),
        pm3_env: context.pm3_env.clone(),
        runtime_dir: context.runtime_dir.clone(),
        uid: context.uid,
        binary: Ok(destination.to_path_buf()),
    }
}

async fn wait_for_takeover(
    session: &ServiceSession,
    programs: &UnitProgramSet,
    backup: &Path,
    emit: &(dyn Fn(&str) + Send + Sync),
) -> Result<()> {
    let supervised = match session.spec.kind {
        UnitKind::Systemd | UnitKind::WinSchtasks => {
            wait_until_supervised(session, programs).await?
        }
        UnitKind::Launchd => {
            if wait_until_supervised(session, programs).await? {
                true
            } else {
                hand_back_to_manager(&session.spec, programs, session.command_timeout_ms).await?;
                wait_until_supervised(session, programs).await?
            }
        }
    };
    if !supervised {
        return Err(Error::InstallTakeover {
            timeout_ms: session.start_timeout_ms,
            backup: backup.to_string_lossy().into_owned(),
        });
    }
    emit(&format!(
        "service {} ({}) is running",
        session.spec.label,
        session.spec.kind.as_str()
    ));
    Ok(())
}

async fn wait_until_supervised(
    session: &ServiceSession,
    programs: &UnitProgramSet,
) -> Result<bool> {
    let interval = Duration::from_millis(session.daemon_poll_interval_ms.max(1));
    let budget = Duration::from_millis(session.start_timeout_ms);
    let started = Instant::now();
    loop {
        if takeover_state(session, programs).await? {
            return Ok(true);
        }
        if started.elapsed() >= budget {
            return Ok(false);
        }
        tokio::time::sleep(interval).await;
    }
}

async fn takeover_state(session: &ServiceSession, programs: &UnitProgramSet) -> Result<bool> {
    let status = query_status(&session.spec, programs, session.command_timeout_ms).await?;
    let filed = read_pid_file(&session.paths).await;
    let supervised = supervised_pid_of(session, programs, filed).await?;
    let client = UdsClient::new(session.paths.socket.clone(), session.request_timeout_ms);
    let healthy = client.daemon_is_healthy().await;
    Ok(takeover_satisfied(status, supervised, filed, healthy))
}

async fn supervised_pid_of(
    session: &ServiceSession,
    programs: &UnitProgramSet,
    filed: Option<u32>,
) -> Result<Option<u32>> {
    match session.spec.kind {
        UnitKind::WinSchtasks => Ok(filed),
        UnitKind::Launchd | UnitKind::Systemd => {
            Ok(query_supervised_pid(&session.spec, programs, session.command_timeout_ms).await?)
        }
    }
}

fn takeover_satisfied(
    status: UnitStatus,
    supervised: Option<u32>,
    filed: Option<u32>,
    healthy: bool,
) -> bool {
    status == UnitStatus::Running && healthy && supervised.is_some() && supervised == filed
}

fn log_step(action: &'static str, from: &Path, to: &Path) {
    let from = from.display().to_string();
    let to = to.display().to_string();
    tracing::debug!(
        feature = "install",
        action,
        from,
        to,
        "pm3 install finished a step"
    );
}

#[expect(clippy::print_stdout, reason = "CLI command output")]
fn emit(line: &str) {
    println!("{line}");
}

#[cfg(test)]
#[path = "tests/install_tests.rs"]
mod tests;
