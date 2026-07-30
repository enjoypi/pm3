use entities::{ProcessStatus, topo_sort};

use crate::{
    Ports, Result,
    persist::save_table,
    record::ProcessRecord,
    start::{StartOutcome, start_one},
    table::ProcessTable,
};

pub async fn resurrect(
    table: &mut ProcessTable,
    logs_dir: &str,
    ports: &impl Ports,
) -> Result<Vec<StartOutcome>> {
    let stored = ports.load().await?;
    let revive: Vec<String> = stored
        .iter()
        .filter(|record| was_supposed_to_run(record))
        .map(|record| record.runtime.name.clone())
        .collect();

    *table = ProcessTable::from_records(stored.into_iter().map(detach_runtime).collect());

    let order = topo_sort(&table.dependency_nodes())?;
    let mut outcomes = Vec::with_capacity(revive.len());
    for name in order {
        if !revive.contains(&name) {
            continue;
        }
        outcomes.push(start_one(table, &name, logs_dir, ports).await?);
    }
    save_table(table, ports).await?;
    Ok(outcomes)
}

const fn was_supposed_to_run(record: &ProcessRecord) -> bool {
    record.runtime.status.is_running() || record.runtime.status.is_shutting_down()
}

const fn detach_runtime(mut record: ProcessRecord) -> ProcessRecord {
    record.runtime.mark_exited(ProcessStatus::Stopped);
    record.runtime.pending_restart = false;
    record
}

#[cfg(test)]
#[path = "tests/resurrect_tests.rs"]
mod tests;
