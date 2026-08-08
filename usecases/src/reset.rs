use crate::{
    Ports, Result, UsecaseError, persist::save_table, selector::AppSelector, table::ProcessTable,
};

pub async fn reset_app(
    table: &mut ProcessTable,
    selector: &AppSelector,
    ports: &impl Ports,
) -> Result<String> {
    let record = table
        .find_mut(selector)
        .ok_or_else(|| UsecaseError::NotFound(selector.to_string()))?;
    record.runtime.reset_restarts();
    let name = record.runtime.name.clone();
    save_table(table, ports).await?;
    log_reset(&name);
    Ok(name)
}

fn log_reset(app: &str) {
    tracing::info!(
        feature = "lifecycle",
        action = "reset",
        app,
        "pm3 cleared a service's restart counters",
    );
}

#[cfg(test)]
#[path = "tests/reset_tests.rs"]
mod tests;
