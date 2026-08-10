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
    let name = table
        .find(selector)
        .map(|record| record.runtime.name.clone())
        .ok_or_else(|| UsecaseError::NotFound(selector.to_string()))?;
    let dependents = dependents_of(table, &name);
    if !dependents.is_empty() {
        return Err(UsecaseError::StillDependedOn { name, dependents });
    }
    let record = table
        .find_mut(selector)
        .expect("internal error: the same selector just located this record");
    let stopped = request_stop(record, ports).await?;
    let removed = table
        .remove(selector)
        .expect("internal error: the same selector just located this record");
    if let Err(error) = save_table(table, ports).await {
        table.restore(removed);
        return Err(error);
    }
    Ok(DeleteOutcome {
        name: stopped.name,
        force_kill_pid: stopped.force_kill_pid,
    })
}

fn dependents_of(table: &ProcessTable, name: &str) -> Vec<String> {
    table
        .records()
        .iter()
        .filter(|record| record.spec.depends_on.iter().any(|dep| dep == name))
        .map(|record| record.runtime.name.clone())
        .collect()
}

#[cfg(test)]
#[path = "tests/delete_tests.rs"]
mod tests;
