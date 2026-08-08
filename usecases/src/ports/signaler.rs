use std::future::Future;

use thiserror::Error;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SignalScope {
    ProcessGroup,
    SinglePid,
}

#[derive(Debug, Eq, PartialEq, Error)]
pub enum SignalError {
    #[error("cannot signal pid {pid}: {reason}")]
    Delivery { pid: u32, reason: String },
}

pub trait Signaler: Send + Sync {
    fn terminate(
        &self,
        pid: u32,
        scope: SignalScope,
    ) -> impl Future<Output = Result<(), SignalError>> + Send;
    fn force_kill(
        &self,
        pid: u32,
        scope: SignalScope,
    ) -> impl Future<Output = Result<(), SignalError>> + Send;
    fn deliver(
        &self,
        signal: &str,
        pid: u32,
        scope: SignalScope,
    ) -> impl Future<Output = Result<(), SignalError>> + Send;
}

impl SignalScope {
    #[must_use]
    pub const fn reaches_the_group(self) -> bool {
        matches!(self, Self::ProcessGroup)
    }
}

#[cfg(test)]
#[path = "../tests/ports_signaler_tests.rs"]
mod tests;
