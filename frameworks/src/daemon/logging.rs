use adapters::{ProcessStatus, UsecaseError};

pub(super) fn log_settled(app: &str, status: ProcessStatus) {
    let status = status.as_str();
    tracing::debug!(
        feature = "supervisor",
        action = "settled",
        app,
        status,
        "managed app settled",
    );
}

pub(super) fn log_stale_restart(app: &str) {
    tracing::debug!(
        feature = "supervisor",
        action = "restart",
        app,
        "pm3 daemon dropped a restart that was cancelled while it was waiting",
    );
}

pub(super) fn log_spared_force_kill(app: &str, pid: u32) {
    tracing::warn!(
        feature = "supervisor",
        action = "force_kill",
        app,
        pid,
        "pm3 daemon spared a pid the kernel handed to another process",
    );
}

pub(super) fn log_stuck_force_kill(app: &str, pid: u32, reason: &str) {
    tracing::warn!(
        feature = "supervisor",
        action = "force_kill",
        app,
        pid,
        reason,
        "pm3 daemon cannot force kill a process, so it may outlive the service",
    );
}

pub(super) fn log_handover(draining: usize) {
    tracing::debug!(
        feature = "lifecycle",
        action = "shutdown",
        draining,
        "pm3 daemon left the services it was told to stop for the next daemon to settle",
    );
}

pub(super) fn log_partial_start(refused: &[String], error: &UsecaseError) {
    let apps = refused.join(",");
    let reason = error.to_string();
    tracing::warn!(
        feature = "lifecycle",
        action = "start",
        apps,
        reason,
        "pm3 daemon started part of the batch and keeps the service files of what it started",
    );
}

pub(super) fn log_failure(action: &str, app: &str, error: &UsecaseError) {
    let reason = error.to_string();
    tracing::warn!(
        feature = "supervisor",
        action,
        app,
        reason,
        "pm3 daemon cannot finish a supervision step",
    );
}
