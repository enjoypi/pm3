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
        pm_id: u32,
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

    if !record.runtime.status.is_running() {
        let name = record.runtime.name.clone();
        let started = start_one(table, &name, logs_dir, ports).await?;
        save_table(table, ports).await?;
        return Ok(RestartOutcome::Started(started));
    }

    record.runtime.request_restart();
    let stopped = request_stop(record, ports).await?;
    save_table(table, ports).await?;
    Ok(RestartOutcome::AwaitingExit {
        pm_id: stopped.pm_id,
        name: stopped.name,
        force_kill_pid: stopped.force_kill_pid,
    })
}

#[cfg(test)]
#[path = "tests/restart_tests.rs"]
mod tests;
