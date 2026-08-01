use entities::{ProcessStatus, topo_sort};

use crate::{
    Ports, Result, UsecaseError, persist::save_table, record::ProcessRecord, selector::AppSelector,
    table::ProcessTable,
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
    let outcome = request_stop(record, ports).await?;
    save_table(table, ports).await?;
    Ok(outcome)
}

pub async fn stop_all_apps(table: &mut ProcessTable, ports: &impl Ports) -> Result<Vec<String>> {
    let order = shutdown_order(table);
    let mut stopped = Vec::with_capacity(order.len());
    for name in order.iter().rev() {
        let record = table
            .find_by_name_mut(name)
            .expect("internal error: the topological order only names records the table holds");
        record.runtime.disarm_schedule();
        if record.runtime.status.is_settled() {
            continue;
        }
        match request_stop(record, ports).await {
            Ok(outcome) => stopped.push(outcome.name),
            Err(error) => log_refused_stop(name, &error),
        }
    }
    save_table(table, ports).await?;
    Ok(stopped)
}

fn shutdown_order(table: &ProcessTable) -> Vec<String> {
    match topo_sort(&table.dependency_nodes()) {
        Ok(order) => order,
        Err(error) => {
            log_unordered_shutdown(&UsecaseError::from(error));
            table
                .records()
                .iter()
                .map(|record| record.runtime.name.clone())
                .collect()
        }
    }
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

pub async fn settle_stopping_apps(
    table: &mut ProcessTable,
    ports: &impl Ports,
) -> Result<Vec<String>> {
    let mut settled = Vec::new();
    for record in table.records_mut() {
        if record.runtime.status != ProcessStatus::Stopping {
            continue;
        }
        record.runtime.mark_exited(ProcessStatus::Stopped);
        settled.push(record.runtime.name.clone());
    }
    save_table(table, ports).await?;
    Ok(settled)
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

    record.runtime.mark_stopping();
    ports.terminate(pid).await?;
    Ok(StopOutcome {
        name,
        force_kill_pid: Some(pid),
    })
}

#[cfg(test)]
#[path = "tests/stop_tests.rs"]
mod tests;
