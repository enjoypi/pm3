pub trait Scheduler: Send + Sync {
    fn next_fire_ms(&self, cron: &str, after_ms: u64) -> Option<u64>;
}
