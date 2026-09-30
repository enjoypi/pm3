use crate::{
    Ports, Result, UsecaseError,
    persist::save_table,
    selector::AppSelector,
    stop::{StopOutcome, request_stop},
    table::ProcessTable,
};

pub async fn delete_app(
    table: &mut ProcessTable,
    selector: &AppSelector,
    ports: &impl Ports,
) -> Result<StopOutcome> {
    let name = table
        .require(selector)
        .map(|record| record.runtime.name.clone())?;
    let dependents = dependents_of(table, &name);
    if !dependents.is_empty() {
        return Err(UsecaseError::StillDependedOn { name, dependents });
    }
    delete_one(table, &name, ports).await
}

pub(crate) async fn delete_one(
    table: &mut ProcessTable,
    name: &str,
    ports: &impl Ports,
) -> Result<StopOutcome> {
    let selector = AppSelector::Name(name.to_string());
    let record = table
        .find_mut(&selector)
        .expect("internal error: the caller only names records the table holds");
    let stopped = request_stop(record, ports).await?;
    let removed = table
        .remove(&selector)
        .expect("internal error: the same selector just located this record");
    if let Err(error) = save_table(table, ports).await {
        table.restore(removed);
        return Err(error);
    }
    Ok(stopped)
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
