use entities::ProcessStatus;

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

pub(crate) async fn request_stop(
    record: &mut ProcessRecord,
    ports: &impl Ports,
) -> Result<StopOutcome> {
    let pm_id = record.runtime.pm_id;
    let name = record.runtime.name.clone();

    if !record.runtime.status.is_running() {
        record.runtime.mark_exited(ProcessStatus::Stopped);
        return Ok(StopOutcome {
            pm_id,
            name,
            force_kill_pid: None,
        });
    }

    let Some(pid) = record.runtime.pid else {
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
