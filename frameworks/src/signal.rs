#[cfg(unix)]
use std::io;

use thiserror::Error;
#[cfg(unix)]
use tokio::signal::unix::{Signal, SignalKind, signal};
#[cfg(windows)]
use tokio::signal::windows::{CtrlC, CtrlShutdown, ctrl_c, ctrl_shutdown};

#[cfg(unix)]
const SIGNAL_INTERRUPT: &str = "SIGINT";
#[cfg(unix)]
const SIGNAL_TERMINATE: &str = "SIGTERM";
#[cfg(windows)]
const EVENT_INTERRUPT: &str = "CTRL_C";
#[cfg(windows)]
const EVENT_SHUTDOWN: &str = "CTRL_SHUTDOWN";

#[derive(Debug, Error)]
pub enum SignalRegisterError {
    #[error("cannot register the {signal} handler: {reason}")]
    Register {
        signal: &'static str,
        reason: String,
    },
}

#[cfg(unix)]
#[derive(Debug)]
pub struct ShutdownSignals {
    interrupt: Signal,
    terminate: Signal,
}

#[cfg(windows)]
#[derive(Debug)]
pub struct ShutdownSignals {
    interrupt: CtrlC,
    shutdown: CtrlShutdown,
}

#[cfg(unix)]
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

#[cfg(windows)]
impl ShutdownSignals {
    pub fn register() -> Result<Self, SignalRegisterError> {
        let interrupt = ctrl_c().map_err(|e| SignalRegisterError::Register {
            signal: EVENT_INTERRUPT,
            reason: e.to_string(),
        })?;
        let shutdown = ctrl_shutdown().map_err(|e| SignalRegisterError::Register {
            signal: EVENT_SHUTDOWN,
            reason: e.to_string(),
        })?;
        Ok(Self {
            interrupt,
            shutdown,
        })
    }

    pub async fn wait(mut self) {
        loop {
            tokio::select! {
                _ = self.interrupt.recv() => log_signal(EVENT_INTERRUPT, "ignored"),
                _ = self.shutdown.recv() => {
                    log_signal(EVENT_SHUTDOWN, "ok");
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
