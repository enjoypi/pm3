use std::future::Future;

pub trait ProcessProbe: Send + Sync {
    fn identity(&self, pid: u32) -> impl Future<Output = Option<String>> + Send;
}
