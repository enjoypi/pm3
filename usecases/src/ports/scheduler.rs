pub trait Scheduler: Send + Sync {
    fn next_fire_ms(&self, app: &str, cron: &str, after_ms: u64) -> Option<u64>;
}
