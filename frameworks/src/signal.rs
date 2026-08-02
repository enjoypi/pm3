use std::io;

use thiserror::Error;
use tokio::signal::unix::{Signal, SignalKind, signal};

const SIGNAL_INTERRUPT: &str = "SIGINT";
const SIGNAL_TERMINATE: &str = "SIGTERM";

#[derive(Debug, Error)]
pub enum SignalRegisterError {
    #[error("cannot register the {signal} handler: {reason}")]
    Register {
        signal: &'static str,
        reason: String,
    },
}

#[derive(Debug)]
pub struct ShutdownSignals {
    interrupt: Signal,
    terminate: Signal,
}

impl ShutdownSignals {
    pub fn register() -> Result<Self, SignalRegisterError> {
        Self::register_with(&signal)
    }

    fn register_with(
        register: &dyn Fn(SignalKind) -> io::Result<Signal>,
    ) -> Result<Self, SignalRegisterError> {
        let interrupt =
            register(SignalKind::interrupt()).map_err(|e| SignalRegisterError::Register {
                signal: SIGNAL_INTERRUPT,
                reason: e.to_string(),
            })?;
        let terminate =
            register(SignalKind::terminate()).map_err(|e| SignalRegisterError::Register {
                signal: SIGNAL_TERMINATE,
                reason: e.to_string(),
            })?;
        Ok(Self {
            interrupt,
            terminate,
        })
    }

    pub async fn wait(mut self) {
        loop {
            tokio::select! {
                _ = self.interrupt.recv() => log_signal(SIGNAL_INTERRUPT, "ignored"),
                _ = self.terminate.recv() => {
                    log_signal(SIGNAL_TERMINATE, "ok");
                    return;
                }
            }
        }
    }
}

fn log_signal(signal: &str, result: &str) {
    tracing::info!(
        feature = "lifecycle",
        action = "shutdown_signal",
        result,
        signal,
        "pm3 daemon received a signal",
    );
}

#[cfg(test)]
#[path = "tests/signal_tests.rs"]
mod tests;
