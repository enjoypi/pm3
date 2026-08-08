use std::{collections::BTreeMap, future::Future};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Liveness {
    Alive(String),
    Gone,
    Unreadable,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ResourceSample {
    pub rss_kib: u64,
    pub cpu_tenths: u32,
}

pub trait ProcessProbe: Send + Sync {
    fn identity(&self, pid: u32) -> impl Future<Output = Liveness> + Send;
    fn wait_gone(&self, pid: u32, timeout_ms: u64) -> impl Future<Output = Liveness> + Send;
    fn resident_memory(&self, pids: &[u32]) -> impl Future<Output = BTreeMap<u32, u64>> + Send;
    fn resource_usage(
        &self,
        pids: &[u32],
    ) -> impl Future<Output = BTreeMap<u32, ResourceSample>> + Send;
}

impl Liveness {
    #[must_use]
    pub fn into_token(self) -> Option<String> {
        match self {
            Self::Alive(token) => Some(token),
            Self::Gone | Self::Unreadable => None,
        }
    }
}

#[cfg(test)]
#[path = "../tests/ports_probe_tests.rs"]
mod tests;
