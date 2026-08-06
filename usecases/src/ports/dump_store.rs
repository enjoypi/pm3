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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StrandedProcess {
    pub name: String,
    pub pid: Option<u32>,
    pub token: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DumpContents {
    pub records: Vec<ProcessRecord>,
    pub stranded: Vec<StrandedProcess>,
    pub boot: Option<String>,
}

pub trait DumpStore: Send + Sync {
    fn load(&self) -> impl Future<Output = Result<DumpContents, DumpError>> + Send;

    fn save(
        &self,
        records: &[ProcessRecord],
        boot: Option<&str>,
    ) -> impl Future<Output = Result<(), DumpError>> + Send;
}

#[cfg(test)]
#[path = "../tests/ports_dump_store_tests.rs"]
mod tests;
