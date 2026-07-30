use tokio::signal::unix::{SignalKind, signal};

const SIGNAL_INTERRUPT: &str = "SIGINT";
const SIGNAL_TERMINATE: &str = "SIGTERM";

pub async fn daemon_shutdown_signal() {
    let mut interrupt = signal(SignalKind::interrupt())
        .expect("internal error: SIGINT handler registration is infallible on unix targets");
    let mut terminate = signal(SignalKind::terminate())
        .expect("internal error: SIGTERM handler registration is infallible on unix targets");

    loop {
        tokio::select! {
            _ = interrupt.recv() => log_signal(SIGNAL_INTERRUPT, "ignored"),
            _ = terminate.recv() => {
                log_signal(SIGNAL_TERMINATE, "ok");
                return;
            }
        }
    }
}

fn log_signal(signal: &str, result: &str) {
    tracing::info!(
        feature = "lifecycle",
        operation = "shutdown.signal",
        result,
        signal,
        "pm3 daemon received a signal",
    );
}

#[cfg(test)]
#[path = "tests/signal_tests.rs"]
mod tests;
