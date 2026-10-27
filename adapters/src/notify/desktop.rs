use std::process::Stdio;

use tokio::process::Command;
use usecases::Alert;

use super::message::render_alert;
use crate::{
    config::NotifyConfig,
    process::{CommandOutcome, capture_timed},
    program::resolve_executable,
};

const NOTIFY_ACTION: &str = "notify";
const OSASCRIPT_SCRIPT: [&str; 6] = [
    "-e",
    "on run argv",
    "-e",
    "display notification (item 2 of argv) with title (item 1 of argv)",
    "-e",
    "end run",
];

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum NotifyBackend {
    Osascript,
    NotifySend,
}

impl NotifyBackend {
    #[must_use]
    pub fn argv(self, title: &str, body: &str) -> Vec<String> {
        let script: &[&str] = match self {
            Self::Osascript => &OSASCRIPT_SCRIPT,
            Self::NotifySend => &[],
        };
        let mut argv: Vec<String> = script.iter().map(ToString::to_string).collect();
        argv.extend([title.to_string(), body.to_string()]);
        argv
    }

    fn program(self, config: &NotifyConfig) -> &str {
        match self {
            Self::Osascript => &config.osascript_program,
            Self::NotifySend => &config.notify_send_program,
        }
    }
}

#[derive(Debug)]
pub struct DesktopNotifier {
    backends: Vec<NotifyBackend>,
    config: NotifyConfig,
    timeout_ms: u64,
    search_path: String,
}

impl DesktopNotifier {
    #[must_use]
    pub fn new(
        config: &NotifyConfig,
        preferred: &[NotifyBackend],
        timeout_ms: u64,
        search_path: String,
    ) -> Self {
        let backends = if config.enabled {
            preferred.to_vec()
        } else {
            Vec::new()
        };
        Self {
            backends,
            config: config.clone(),
            timeout_ms,
            search_path,
        }
    }

    pub async fn notify(&self, alert: &Alert) {
        let (title, body) = render_alert(&self.config, alert);
        for backend in &self.backends {
            self.send(*backend, &alert.name, &title, &body).await;
        }
    }

    async fn send(&self, backend: NotifyBackend, app: &str, title: &str, body: &str) {
        let program = backend.program(&self.config);
        let Some(resolved) = resolve_executable(program, Some(&self.search_path)) else {
            log_unsent(app, program, "cannot find the program on pm3.search_path");
            return;
        };
        let mut command = Command::new(resolved);
        command
            .args(backend.argv(title, body))
            .stdin(Stdio::null())
            .kill_on_drop(true);
        let timed = capture_timed(command, self.timeout_ms).await;
        let reason = match timed.outcome {
            CommandOutcome::Finished(output) => {
                if output.status.success() {
                    log_sent(app, program, body, timed.duration_ms);
                    return;
                }
                String::from_utf8_lossy(&output.stderr).into_owned()
            }
            CommandOutcome::Stalled => "the program timed out".to_string(),
            CommandOutcome::SpawnFailed(error) => error.to_string(),
        };
        log_unsent(app, program, &reason);
    }
}

fn log_sent(app: &str, program: &str, body: &str, duration_ms: u128) {
    tracing::info!(
        feature = "supervisor",
        action = NOTIFY_ACTION,
        app,
        program,
        body,
        duration_ms,
        "pm3 raised a desktop notification about a failing service",
    );
}

fn log_unsent(app: &str, program: &str, reason: &str) {
    tracing::warn!(
        feature = "supervisor",
        action = NOTIFY_ACTION,
        app,
        program,
        reason,
        "pm3 cannot raise a desktop notification, so the operator only learns of this failure from the logs",
    );
}

#[cfg(test)]
#[path = "../tests/notify_desktop_tests.rs"]
mod tests;
