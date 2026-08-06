use std::{future::Future, pin::Pin, sync::Arc, time::Duration};

use adapters::{
    DaemonHandle, Pm3Paths, SandboxProgramSet, SpecSource, load_and_parse_config,
    log_startup_banner, router,
};
use tokio::sync::mpsc;

use super::{
    actor::Daemon,
    events::DaemonEvent,
    ports::DaemonPorts,
    runner::run,
    socket::{BindOutcome, OwnerOnlyListener, bind_uds},
};
use crate::{
    Error, Result,
    layout::{
        clear_runtime_files, ensure_layout, host_home, resolve_cfg_dir, resolve_layout,
        write_pid_file,
    },
    sandbox_probe::detect_host_backend,
    server::serve_listener,
    signal::{ShutdownSignals, SignalRegisterError},
    telemetry::{LogSink, init_telemetry},
};

type ShutdownFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

const TMPDIR_VARIABLE: &str = "TMPDIR";

pub async fn run_daemon(config_path: &str) -> Result<()> {
    run_daemon_with_signals(config_path, ShutdownSignals::register()).await
}

async fn run_daemon_with_signals(
    config_path: &str,
    signals: std::result::Result<ShutdownSignals, SignalRegisterError>,
) -> Result<()> {
    let signals = signals?;
    run_daemon_with_shutdown(config_path, Box::pin(signals.wait())).await
}

pub async fn run_daemon_with_shutdown(config_path: &str, shutdown: ShutdownFuture) -> Result<()> {
    let config = load_and_parse_config(config_path)?;
    init_telemetry(&config.telemetry, LogSink::Stdout)
        .expect("internal error: load_and_parse_config validated log_level and log_format");
    let home = host_home();
    let paths = resolve_layout(&config.pm3, home.as_deref())?;
    let cfg_dir = resolve_cfg_dir(&config.pm3, home.as_deref())?;
    ensure_layout(&paths, &cfg_dir).await?;
    let BindOutcome::Bound(listener) = bind_uds(&paths.socket).await? else {
        return Ok(());
    };
    write_pid_file(&paths).await?;
    log_startup_banner(
        &config,
        env!("CARGO_PKG_VERSION"),
        &paths.socket.to_string_lossy(),
    );
    let specs = SpecSource {
        cfg_dir,
        config: config.pm3.clone(),
        home_dir: paths.root.to_string_lossy().into_owned(),
        host_home: home,
        logs_dir: paths.logs_dir.to_string_lossy().into_owned(),
        tmp_dir: std::env::var(TMPDIR_VARIABLE).ok(),
    };
    let served = serve_supervised(specs, &paths, listener, shutdown).await;
    clear_runtime_files(&paths).await;
    served
}

async fn serve_supervised(
    specs: SpecSource,
    paths: &Pm3Paths,
    listener: OwnerOnlyListener,
    shutdown: ShutdownFuture,
) -> Result<()> {
    let sandbox_programs = SandboxProgramSet::from_config(&specs.config.sandbox);
    let backend = detect_host_backend(&sandbox_programs, &specs.config.search_path);
    let ports = Arc::new(DaemonPorts::new(
        paths.dump_file.clone(),
        specs.clone(),
        backend,
    ));
    let channel_depth = specs.config.daemon_channel_depth;
    let (commands, command_queue) = mpsc::channel(channel_depth);
    let (events, event_queue) = mpsc::channel(channel_depth);
    let drain_timeout = Duration::from_secs(specs.config.drain_timeout_secs);
    let body_limit_bytes = specs.config.request_body_limit_bytes;
    let mut daemon = Daemon::new(specs, ports, events.clone());
    daemon.resurrect_saved_apps().await;

    let supervisor = tokio::spawn(run(daemon, command_queue, event_queue));
    let served = serve_listener(
        listener,
        router(DaemonHandle::new(commands.clone()), body_limit_bytes),
        shutdown,
        drain_timeout,
    )
    .await;
    events.send(DaemonEvent::Shutdown).await.ok();
    supervisor.await.ok();
    served.map_err(Error::from)
}

#[cfg(test)]
#[path = "../tests/daemon_service_tests.rs"]
mod tests;
