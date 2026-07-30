use crate::{
    Result, UsecaseError, record::ProcessView, selector::AppSelector, table::ProcessTable,
};

#[must_use]
pub fn list_apps(table: &ProcessTable, now_ms: u64) -> Vec<ProcessView> {
    table
        .records()
        .iter()
        .map(|record| record.view(now_ms))
        .collect()
}

pub fn describe_app(
    table: &ProcessTable,
    selector: &AppSelector,
    now_ms: u64,
) -> Result<ProcessView> {
    table
        .find(selector)
        .map(|record| record.view(now_ms))
        .ok_or_else(|| UsecaseError::NotFound(selector.to_string()))
}

#[cfg(test)]
#[path = "tests/query_tests.rs"]
mod tests;
