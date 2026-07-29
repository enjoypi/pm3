const SIGNAL_INTERRUPT: &str = "SIGINT";
#[cfg(unix)]
const SIGNAL_TERMINATE: &str = "SIGTERM";

pub async fn shutdown_signal() {
    let signal = next_shutdown_signal().await;
    tracing::info!(
        feature = "lifecycle",
        operation = "shutdown.signal",
        result = "ok",
        signal,
        "shutdown signal received"
    );
}

#[cfg(unix)]
async fn next_shutdown_signal() -> &'static str {
    use tokio::signal::unix::{SignalKind, signal};

    let mut interrupt = signal(SignalKind::interrupt())
        .expect("internal error: SIGINT handler registration is infallible on unix targets");
    let mut terminate = signal(SignalKind::terminate())
        .expect("internal error: SIGTERM handler registration is infallible on unix targets");

    tokio::select! {
        _ = interrupt.recv() => SIGNAL_INTERRUPT,
        _ = terminate.recv() => SIGNAL_TERMINATE,
    }
}

#[cfg(not(unix))]
async fn next_shutdown_signal() -> &'static str {
    tokio::signal::ctrl_c()
        .await
        .expect("internal error: SIGINT handler registration is infallible on supported targets");
    SIGNAL_INTERRUPT
}
