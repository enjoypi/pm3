use crate::{
    Ports, Result, UsecaseError,
    persist::save_table,
    selector::AppSelector,
    start::{StartOutcome, start_one},
    stop::request_stop,
    table::ProcessTable,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RestartOutcome {
    Started(StartOutcome),
    AwaitingExit {
        name: String,
        force_kill_pid: Option<u32>,
    },
}

pub async fn restart_app(
    table: &mut ProcessTable,
    selector: &AppSelector,
    logs_dir: &str,
    ports: &impl Ports,
) -> Result<RestartOutcome> {
    let record = table
        .find_mut(selector)
        .ok_or_else(|| UsecaseError::NotFound(selector.to_string()))?;

    if record.runtime.status.is_settled() {
        let name = record.runtime.name.clone();
        let started = start_one(table, &name, logs_dir, ports).await?;
        persist_restart(table, &name, ports).await;
        return Ok(RestartOutcome::Started(started));
    }

    record.runtime.request_restart();
    let stopped = request_stop(record, ports).await?;
    persist_restart(table, &stopped.name, ports).await;
    Ok(RestartOutcome::AwaitingExit {
        name: stopped.name,
        force_kill_pid: stopped.force_kill_pid,
    })
}

async fn persist_restart(table: &ProcessTable, app: &str, ports: &impl Ports) {
    if let Err(error) = save_table(table, ports).await {
        log_unsaved_restart(app, &error);
    }
}

fn log_unsaved_restart(app: &str, error: &UsecaseError) {
    let reason = error.to_string();
    tracing::warn!(
        feature = "lifecycle",
        action = "restart",
        app,
        reason,
        "pm3 cannot persist the process table after restarting, so a daemon restart may lose this service",
    );
}

#[cfg(test)]
#[path = "tests/restart_tests.rs"]
mod tests;
