use entities::ProcessStatus;

use crate::{
    Ports, Result, SignalScope, UsecaseError,
    persist::save_table,
    record::ProcessRecord,
    selector::AppSelector,
    table::{ProcessTable, dependency_order},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StopOutcome {
    pub name: String,
    pub force_kill_pid: Option<u32>,
}

pub async fn stop_app(
    table: &mut ProcessTable,
    selector: &AppSelector,
    ports: &impl Ports,
) -> Result<StopOutcome> {
    let record = table
        .find_mut(selector)
        .ok_or_else(|| UsecaseError::NotFound(selector.to_string()))?;
    record.runtime.disarm_schedule();
    record.runtime.cancel_restart();
    let outcome = request_stop(record, ports).await?;
    save_table(table, ports).await?;
    Ok(outcome)
}

pub async fn stop_all_apps(
    table: &mut ProcessTable,
    ports: &impl Ports,
) -> Result<Vec<StopOutcome>> {
    let order = dependency_order(table, log_unordered_shutdown);
    let mut stopped = Vec::with_capacity(order.len());
    for name in order.iter().rev() {
        let record = table
            .find_by_name_mut(name)
            .expect("internal error: the topological order only names records the table holds");
        record.runtime.disarm_schedule();
        record.runtime.cancel_restart();
        if record.runtime.status.is_settled() {
            continue;
        }
        match request_stop(record, ports).await {
            Ok(outcome) => stopped.push(outcome),
            Err(error) => log_refused_stop(name, &error),
        }
    }
    save_table(table, ports).await?;
    Ok(stopped)
}

fn log_unordered_shutdown(error: &UsecaseError) {
    let reason = error.to_string();
    tracing::warn!(
        feature = "lifecycle",
        action = "stop_all",
        reason,
        "pm3 cannot order the shutdown, so it stops every service in table order",
    );
}

pub async fn persist_for_handover(table: &ProcessTable, ports: &impl Ports) -> Result<Vec<String>> {
    let draining = table
        .records()
        .iter()
        .filter(|record| record.runtime.status.is_shutting_down())
        .map(|record| record.runtime.name.clone())
        .collect();
    save_table(table, ports).await?;
    Ok(draining)
}

fn log_refused_stop(app: &str, error: &UsecaseError) {
    let reason = error.to_string();
    tracing::warn!(
        feature = "lifecycle",
        action = "stop_all",
        app,
        reason,
        "pm3 cannot stop a service while stopping every service",
    );
}

pub(crate) async fn request_stop(
    record: &mut ProcessRecord,
    ports: &impl Ports,
) -> Result<StopOutcome> {
    let name = record.runtime.name.clone();

    let live = record
        .runtime
        .pid
        .filter(|_pid| !record.runtime.status.is_settled());
    let Some(pid) = live else {
        record.runtime.mark_exited(ProcessStatus::Stopped);
        return Ok(StopOutcome {
            name,
            force_kill_pid: None,
        });
    };

    ports.terminate(pid, SignalScope::ProcessGroup).await?;
    record.runtime.mark_stopping();
    log_stopping(&name, pid);
    Ok(StopOutcome {
        name,
        force_kill_pid: Some(pid),
    })
}

fn log_stopping(app: &str, pid: u32) {
    tracing::info!(
        feature = "lifecycle",
        action = "stop",
        app,
        pid,
        "pm3 asked a service to stop",
    );
}

#[cfg(test)]
#[path = "tests/stop_tests.rs"]
mod tests;
