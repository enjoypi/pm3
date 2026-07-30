use std::{future::Future, pin::Pin, sync::Arc, time::Duration};

use adapters::{
    AppConfig, DaemonHandle, Pm3Paths, load_and_parse_config, log_startup_banner, logs_dir_of,
    router,
};
use tokio::{net::UnixListener, sync::mpsc};

use super::{
    actor::{self, Daemon, DaemonEvent},
    ports::DaemonPorts,
    socket::{BindOutcome, bind_uds},
};
use crate::{
    Error, Result,
    layout::{clear_runtime_files, ensure_layout, host_home, resolve_layout, write_pid_file},
    sandbox_probe::detect_host_backend,
    server::serve_listener,
    signal::daemon_shutdown_signal,
    telemetry::init_telemetry,
};

type ShutdownFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

const CHANNEL_DEPTH: usize = 32;
const TMPDIR_VARIABLE: &str = "TMPDIR";

pub async fn run_daemon(config_path: &str) -> Result<()> {
    run_daemon_with_shutdown(config_path, Box::pin(daemon_shutdown_signal())).await
}

pub async fn run_daemon_with_shutdown(config_path: &str, shutdown: ShutdownFuture) -> Result<()> {
    let config = load_and_parse_config(config_path)?;
    init_telemetry(&config.telemetry)
        .expect("internal error: load_and_parse_config validated log_level and log_format");
    let paths = resolve_layout(&config.pm3, host_home().as_deref())?;
    ensure_layout(&paths).await?;
    let BindOutcome::Bound(listener) = bind_uds(&paths.socket).await? else {
        return Ok(());
    };
    write_pid_file(&paths).await?;
    log_startup_banner(
        &config,
        env!("CARGO_PKG_VERSION"),
        &paths.socket.to_string_lossy(),
    );
    let served = serve_supervised(&config, &paths, listener, shutdown).await;
    clear_runtime_files(&paths).await;
    served
}

async fn serve_supervised(
    config: &AppConfig,
    paths: &Pm3Paths,
    listener: UnixListener,
    shutdown: ShutdownFuture,
) -> Result<()> {
    let ports = Arc::new(DaemonPorts::new(
        paths.dump_file.clone(),
        detect_host_backend(),
    ));
    let (commands, command_queue) = mpsc::channel(CHANNEL_DEPTH);
    let (events, event_queue) = mpsc::channel(CHANNEL_DEPTH);
    let mut daemon = Daemon::new(
        config.pm3.clone(),
        logs_dir_of(&paths.root),
        std::env::var(TMPDIR_VARIABLE).ok(),
        ports,
        events.clone(),
    );
    daemon.resurrect_saved_apps().await;

    let supervisor = tokio::spawn(actor::run(daemon, command_queue, event_queue));
    let drain_timeout = Duration::from_secs(config.pm3.drain_timeout_secs);
    let served = serve_listener(
        listener,
        router(DaemonHandle::new(commands.clone())),
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
