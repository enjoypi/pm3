use std::str::FromStr as _;

use chrono::{
    DateTime, Datelike as _, Days, DurationRound as _, Local, Months, NaiveDate, NaiveDateTime,
    NaiveTime, TimeDelta, TimeZone as _,
};
use croner::Cron;
use thiserror::Error;
use usecases::Scheduler;

use super::random_expand::{ExpandError, expand_random, widen_random};

const MILLIS_PER_SECOND: u64 = 1000;
const RANDOM_MARK: char = '~';
const FIELD_WINDOWS: [Window; 5] = [
    Window::Hour,
    Window::Day,
    Window::Month,
    Window::Year,
    Window::Week,
];
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0100_0000_01b3;
const SEED_SEPARATOR: u8 = 0xff;

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
    fn next_fire_ms(&self, app: &str, cron: &str, after_ms: u64) -> Option<u64> {
        let template = Cron::from_str(&widen_random(cron).ok()?).ok()?;
        let window = Window::of(cron);
        let after = i64::try_from(after_ms - after_ms % MILLIS_PER_SECOND).ok()?;
        let mut cursor = DateTime::from_timestamp_millis(after)?.with_timezone(&Local);
        let mut inclusive = false;
        loop {
            let candidate = template.find_next_occurrence(&cursor, inclusive).ok()?;
            let start = window.start_of(candidate.naive_local());
            let fire = drawn(app, cron, start)
                .find_next_occurrence(&cursor, inclusive)
                .ok()
                .filter(|fire| window.start_of(fire.naive_local()) == start);
            if let Some(fire) = fire {
                return Some(fire.timestamp_millis().max(0).cast_unsigned());
            }
            cursor = window
                .end_of(start)
                .and_then(|end| Local.from_local_datetime(&end).earliest())
                .unwrap_or(candidate + TimeDelta::seconds(1));
            inclusive = true;
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Window {
    Hour,
    Day,
    Week,
    Month,
    Year,
}

impl Window {
    fn of(cron: &str) -> Self {
        cron.split_whitespace()
            .zip(FIELD_WINDOWS)
            .filter(|(field, _window)| field.contains(RANDOM_MARK))
            .map(|(_field, window)| window)
            .max()
            .unwrap_or(Self::Hour)
    }

    fn start_of(self, at: NaiveDateTime) -> NaiveDateTime {
        let date = at.date();
        match self {
            Self::Hour => at
                .duration_trunc(TimeDelta::hours(1))
                .expect("internal error: a calendar instant truncates to a whole hour"),
            Self::Day => date.and_time(NaiveTime::MIN),
            Self::Week => (date - Days::new(u64::from(date.weekday().num_days_from_sunday())))
                .and_time(NaiveTime::MIN),
            Self::Month => first_day(date.year(), date.month()).and_time(NaiveTime::MIN),
            Self::Year => first_day(date.year(), 1).and_time(NaiveTime::MIN),
        }
    }

    const fn end_of(self, start: NaiveDateTime) -> Option<NaiveDateTime> {
        match self {
            Self::Hour => start.checked_add_signed(TimeDelta::hours(1)),
            Self::Day => start.checked_add_days(Days::new(1)),
            Self::Week => start.checked_add_days(Days::new(7)),
            Self::Month => start.checked_add_months(Months::new(1)),
            Self::Year => start.checked_add_months(Months::new(12)),
        }
    }
}

const fn first_day(year: i32, month: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, 1)
        .expect("internal error: the first day of a calendar month exists")
}

fn drawn(app: &str, cron: &str, window_start: NaiveDateTime) -> Cron {
    let mut rng = fastrand::Rng::with_seed(window_seed(app, cron, window_start));
    let expanded = expand_random(cron, &mut rng)
        .expect("internal error: widening already validated the random fields");
    Cron::from_str(&expanded).expect("internal error: every draw lies within the widened schedule")
}

fn window_seed(app: &str, cron: &str, window_start: NaiveDateTime) -> u64 {
    let stamp = window_start.and_utc().timestamp().to_le_bytes();
    app.bytes()
        .chain([SEED_SEPARATOR])
        .chain(cron.bytes())
        .chain([SEED_SEPARATOR])
        .chain(stamp)
        .fold(FNV_OFFSET, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(FNV_PRIME)
        })
}

#[cfg(test)]
#[path = "../tests/schedule_cron_scheduler_tests.rs"]
mod tests;
