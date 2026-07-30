use entities::{ProcessStatus, topo_sort};

use crate::{
    Ports, Result, UsecaseError, persist::save_table, record::ProcessRecord, selector::AppSelector,
    table::ProcessTable,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StopOutcome {
    pub pm_id: u32,
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
    let outcome = request_stop(record, ports).await?;
    save_table(table, ports).await?;
    Ok(outcome)
}

pub async fn stop_all_apps(table: &mut ProcessTable, ports: &impl Ports) -> Result<Vec<String>> {
    let order = topo_sort(&table.dependency_nodes()).unwrap_or_default();
    let mut stopped = Vec::with_capacity(order.len());
    for name in order.iter().rev() {
        let record = table
            .find_by_name_mut(name)
            .expect("internal error: the topological order only names records the table holds");
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
    let pm_id = record.runtime.pm_id;
    let name = record.runtime.name.clone();

    let live = record
        .runtime
        .pid
        .filter(|_pid| !record.runtime.status.is_settled());
    let Some(pid) = live else {
        record.runtime.mark_exited(ProcessStatus::Stopped);
        return Ok(StopOutcome {
            pm_id,
            name,
            force_kill_pid: None,
        });
    };

    record.runtime.mark_stopping();
    ports.terminate(pid).await?;
    Ok(StopOutcome {
        pm_id,
        name,
        force_kill_pid: Some(pid),
    })
}

#[cfg(test)]
#[path = "tests/stop_tests.rs"]
mod tests;
