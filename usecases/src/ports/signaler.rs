use std::future::Future;

use thiserror::Error;

#[derive(Debug, Eq, PartialEq, Error)]
pub enum SignalError {
    #[error("cannot signal pid {pid}: {reason}")]
    Delivery { pid: u32, reason: String },
}

pub trait Signaler: Send + Sync {
    fn terminate(&self, pid: u32) -> impl Future<Output = Result<(), SignalError>> + Send;
    fn force_kill(&self, pid: u32) -> impl Future<Output = Result<(), SignalError>> + Send;
}

#[cfg(test)]
#[path = "../tests/ports_signaler_tests.rs"]
mod tests;
