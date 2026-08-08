use std::future::Future;

use entities::ReadyProbe;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Readiness {
    Ready,
    Pending,
    Failed(String),
}

pub trait ReadyProber: Send + Sync {
    fn check_ready(&self, probe: &ReadyProbe) -> impl Future<Output = Readiness> + Send;
}

#[cfg(test)]
#[path = "../tests/ports_ready_tests.rs"]
mod tests;
