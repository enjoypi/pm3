use std::str::FromStr as _;

use chrono::{DateTime, Local};
use croner::Cron;
use thiserror::Error;
use usecases::Scheduler;

use super::random_expand::{ExpandError, expand_random};

const MILLIS_PER_SECOND: u64 = 1000;

#[derive(Copy, Clone, Debug)]
pub struct CronScheduler;

#[derive(Debug, Error)]
pub enum CronError {
    #[error("cannot accept schedule '{expr}' for app '{app}': {source}")]
    Expand {
        app: String,
        expr: String,
        source: ExpandError,
    },

    #[error("cannot parse schedule '{expr}' for app '{app}': {reason}")]
    Parse {
        app: String,
        expr: String,
        reason: String,
    },
}

pub fn validate_cron(app: &str, expr: &str) -> Result<(), CronError> {
    let mut rng = fastrand::Rng::new();
    let expanded = expand_random(expr, &mut rng).map_err(|source| CronError::Expand {
        app: app.to_string(),
        expr: expr.to_string(),
        source,
    })?;
    Cron::from_str(&expanded)
        .map(|_parsed| ())
        .map_err(|error| CronError::Parse {
            app: app.to_string(),
            expr: expr.to_string(),
            reason: error.to_string(),
        })
}

impl Scheduler for CronScheduler {
    fn next_fire_ms(&self, cron: &str, after_ms: u64) -> Option<u64> {
        let mut rng = fastrand::Rng::new();
        let expanded = expand_random(cron, &mut rng).ok()?;
        let schedule = Cron::from_str(&expanded).ok()?;
        let after = i64::try_from(after_ms - after_ms % MILLIS_PER_SECOND).ok()?;
        let start = DateTime::from_timestamp_millis(after)?.with_timezone(&Local);
        let next = schedule.find_next_occurrence(&start, false).ok()?;
        Some(next.timestamp_millis().max(0).cast_unsigned())
    }
}

#[cfg(test)]
#[path = "../tests/schedule_cron_scheduler_tests.rs"]
mod tests;
