use crate::{
    Ports, Result, UsecaseError, persist::save_table, selector::AppSelector, stop::request_stop,
    table::ProcessTable,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeleteOutcome {
    pub name: String,
    pub force_kill_pid: Option<u32>,
}

pub async fn delete_app(
    table: &mut ProcessTable,
    selector: &AppSelector,
    ports: &impl Ports,
) -> Result<DeleteOutcome> {
    let record = table
        .find_mut(selector)
        .ok_or_else(|| UsecaseError::NotFound(selector.to_string()))?;
    let stopped = request_stop(record, ports).await?;
    table.remove(selector);
    save_table(table, ports).await?;
    Ok(DeleteOutcome {
        name: stopped.name,
        force_kill_pid: stopped.force_kill_pid,
    })
}

#[cfg(test)]
#[path = "tests/delete_tests.rs"]
mod tests;
