use entities::{AppSpec, DependencyNode, ProcessRuntime};

use crate::{record::ProcessRecord, selector::AppSelector};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProcessTable {
    records: Vec<ProcessRecord>,
}

impl ProcessTable {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    #[must_use]
    pub const fn from_records(records: Vec<ProcessRecord>) -> Self {
        Self { records }
    }

    #[must_use]
    pub const fn records(&self) -> &[ProcessRecord] {
        self.records.as_slice()
    }

    #[must_use]
    pub fn find(&self, selector: &AppSelector) -> Option<&ProcessRecord> {
        self.records
            .iter()
            .find(|record| selector.matches(record.runtime.pm_id, &record.runtime.name))
    }

    pub fn find_mut(&mut self, selector: &AppSelector) -> Option<&mut ProcessRecord> {
        self.records
            .iter_mut()
            .find(|record| selector.matches(record.runtime.pm_id, &record.runtime.name))
    }

    pub fn find_by_name_mut(&mut self, name: &str) -> Option<&mut ProcessRecord> {
        self.records
            .iter_mut()
            .find(|record| record.runtime.name == name)
    }

    pub fn upsert(&mut self, spec: AppSpec, now_ms: u64) -> u32 {
        if let Some(existing) = self.find_by_name_mut(&spec.name) {
            existing.spec = spec;
            return existing.runtime.pm_id;
        }
        let pm_id = self.next_pm_id();
        let runtime = ProcessRuntime::new(pm_id, spec.name.clone(), now_ms);
        self.records.push(ProcessRecord { spec, runtime });
        pm_id
    }

    pub fn remove(&mut self, selector: &AppSelector) -> Option<ProcessRecord> {
        let index = self
            .records
            .iter()
            .position(|record| selector.matches(record.runtime.pm_id, &record.runtime.name))?;
        Some(self.records.remove(index))
    }

    #[must_use]
    pub fn dependency_nodes(&self) -> Vec<DependencyNode<'_>> {
        self.records
            .iter()
            .map(|record| record.spec.dependency_node())
            .collect()
    }

    fn next_pm_id(&self) -> u32 {
        self.records
            .iter()
            .map(|record| record.runtime.pm_id)
            .max()
            .map_or(0, |highest| highest.saturating_add(1))
    }
}

#[cfg(test)]
#[path = "test_helpers/table_test_helpers.rs"]
mod test_helpers;
#[cfg(test)]
#[path = "tests/table_tests.rs"]
mod tests;
