use crate::{Result, ports::DumpStore, table::ProcessTable};

pub async fn save_table(table: &ProcessTable, store: &impl DumpStore) -> Result<()> {
    store.save(table.records(), table.boot()).await?;
    Ok(())
}
