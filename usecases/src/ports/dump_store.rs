use std::future::Future;

use thiserror::Error;

use crate::record::ProcessRecord;

#[derive(Debug, Eq, PartialEq, Error)]
pub enum DumpError {
    #[error("cannot read state file '{path}': {reason}")]
    Read { path: String, reason: String },

    #[error("cannot write state file '{path}': {reason}")]
    Write { path: String, reason: String },
}

pub trait DumpStore: Send + Sync {
    fn load(&self) -> impl Future<Output = Result<Vec<ProcessRecord>, DumpError>> + Send;

    fn save(&self, records: &[ProcessRecord])
    -> impl Future<Output = Result<(), DumpError>> + Send;
}

#[cfg(test)]
#[path = "../tests/ports_dump_store_tests.rs"]
mod tests;
