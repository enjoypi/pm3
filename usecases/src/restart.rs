use crate::{
    Ports, Result, UsecaseError,
    persist::save_table,
    selector::AppSelector,
    start::{StartOutcome, start_one},
    stop::{StopOutcome, request_stop},
    table::ProcessTable,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RestartOutcome {
    Started(StartOutcome),
    AwaitingExit(StopOutcome),
}

pub async fn restart_app(
    table: &mut ProcessTable,
    selector: &AppSelector,
    logs_dir: &str,
    ports: &impl Ports,
) -> Result<RestartOutcome> {
    let record = table.require_mut(selector)?;

    if record.runtime.status.is_settled() {
        let name = record.runtime.name.clone();
        let started = start_one(table, &name, logs_dir, ports).await?;
        persist_restart(table, &name, ports).await;
        return Ok(RestartOutcome::Started(started));
    }

    let stopped = request_stop(record, ports).await?;
    record.runtime.request_restart();
    persist_restart(table, &stopped.name, ports).await;
    Ok(RestartOutcome::AwaitingExit(stopped))
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
