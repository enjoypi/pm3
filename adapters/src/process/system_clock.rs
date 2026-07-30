use std::time::{SystemTime, UNIX_EPOCH};

use usecases::Clock;

#[derive(Clone, Copy, Debug)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        let since_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        u64::try_from(since_epoch.as_millis()).unwrap_or(u64::MAX)
    }
}

#[cfg(test)]
#[path = "../tests/process_system_clock_tests.rs"]
mod tests;
