use chrono::Timelike as _;

use super::*;

#[test]
fn every_draw_of_a_stepped_weekday_range_stays_parseable() {
    for _attempt in 0..64 {
        validate_cron("nightly", "0 4 * * 5~7/3").expect("croner accepts sunday written as seven");
    }
}

const APP: &str = "nightly";
const MINUTE_MS: u64 = 60_000;
const SOME_INSTANT_MS: u64 = 1_700_000_000_000;

#[test]
fn the_next_occurrence_lands_within_the_coming_minute() {
    let next = CronScheduler
        .next_fire_ms(APP, "* * * * *", SOME_INSTANT_MS)
        .expect("an every-minute schedule always has a next occurrence");
    assert!(next > SOME_INSTANT_MS, "next must lie in the future");
    assert!(
        next - SOME_INSTANT_MS <= MINUTE_MS,
        "next must be at most a minute away: {next}"
    );
}

#[test]
fn a_random_field_still_resolves_to_an_occurrence() {
    let next = CronScheduler
        .next_fire_ms(APP, "~ * * * *", SOME_INSTANT_MS)
        .expect("a random minute still yields an occurrence");
    assert!(next > SOME_INSTANT_MS);
}

#[test]
fn an_unexpandable_schedule_has_no_next_occurrence() {
    assert_eq!(
        CronScheduler.next_fire_ms(APP, "0~59/0 * * * *", SOME_INSTANT_MS),
        None
    );
}

#[test]
fn an_unparsable_schedule_has_no_next_occurrence() {
    assert_eq!(
        CronScheduler.next_fire_ms(APP, "nonsense", SOME_INSTANT_MS),
        None
    );
}

#[test]
fn a_schedule_that_never_matches_has_no_next_occurrence() {
    assert_eq!(
        CronScheduler.next_fire_ms(APP, "0 0 30 2 *", SOME_INSTANT_MS),
        None
    );
}

#[test]
fn a_timestamp_beyond_the_signed_range_has_no_next_occurrence() {
    assert_eq!(CronScheduler.next_fire_ms(APP, "* * * * *", u64::MAX), None);
}

#[test]
fn a_timestamp_outside_the_calendar_has_no_next_occurrence() {
    let beyond_chrono = u64::MAX / 2;
    assert_eq!(
        CronScheduler.next_fire_ms(APP, "* * * * *", beyond_chrono),
        None
    );
}

#[test]
fn validation_accepts_a_plain_schedule() {
    validate_cron("app", "30 9,18 * * *").expect("a standard five-field cron is valid");
}

#[test]
fn validation_accepts_a_random_schedule() {
    validate_cron("app", "25~35 9,18 * * *").expect("a bounded tilde is valid");
}

#[test]
fn validation_reports_the_expansion_failure() {
    let err = validate_cron("sweep", "0~59/0 * * * *").unwrap_err();
    assert!(
        err.to_string().contains("step 0"),
        "expansion detail should survive: {err}"
    );
    assert!(
        err.to_string().contains("sweep"),
        "app name should survive: {err}"
    );
}

#[test]
fn validation_reports_the_parse_failure() {
    let err = validate_cron("sweep", "nonsense").unwrap_err();
    assert!(
        err.to_string().starts_with("cannot parse schedule"),
        "unexpected message: {err}"
    );
}

#[test]
fn every_cron_error_renders_a_message() {
    let errors = [
        CronError::Expand {
            app: "sweep".to_string(),
            expr: "0~59/0 * * * *".to_string(),
            source: ExpandError::ZeroStep {
                field: "0~59/0".to_string(),
            },
        },
        CronError::Parse {
            app: "sweep".to_string(),
            expr: "nonsense".to_string(),
            reason: "bad pattern".to_string(),
        },
    ];
    for err in errors {
        assert!(
            err.to_string().starts_with("cannot"),
            "error message must start with a verb: {err}"
        );
    }
}

#[test]
fn the_next_occurrence_ignores_the_subsecond_part_of_the_asking_instant() {
    let on_the_second = CronScheduler.next_fire_ms(APP, "* * * * *", SOME_INSTANT_MS);
    let late_in_the_second = CronScheduler.next_fire_ms(APP, "* * * * *", SOME_INSTANT_MS + 999);
    assert_eq!(
        on_the_second, late_in_the_second,
        "the same cycle must resolve to one instant whenever it is asked"
    );
}

#[test]
fn the_next_occurrence_lands_on_a_whole_second() {
    let next = CronScheduler
        .next_fire_ms(APP, "* * * * *", SOME_INSTANT_MS + 1)
        .expect("an every-minute schedule always has a next occurrence");
    assert_eq!(next % 1000, 0, "cron resolves to seconds, got: {next}");
}

fn local_ms(year: i32, month: u32, day: u32, hour: u32) -> u64 {
    Local
        .with_ymd_and_hms(year, month, day, hour, 0, 0)
        .earliest()
        .expect("a mid-january hour exists in every zone")
        .timestamp_millis()
        .cast_unsigned()
}

fn local_at(ms: u64) -> NaiveDateTime {
    DateTime::from_timestamp_millis(ms.cast_signed())
        .expect("a fire time lies within the calendar")
        .with_timezone(&Local)
        .naive_local()
}

fn fires(cron: &str, from_ms: u64, count: usize) -> Vec<NaiveDateTime> {
    let mut after = from_ms;
    (0..count)
        .map(|_| {
            after = CronScheduler
                .next_fire_ms(APP, cron, after)
                .expect("the schedule keeps firing");
            local_at(after)
        })
        .collect()
}

fn sunday_of(at: NaiveDateTime) -> NaiveDate {
    at.date() - Days::new(u64::from(at.weekday().num_days_from_sunday()))
}

#[test]
fn a_random_minute_fires_once_in_each_hour() {
    let hours: Vec<NaiveDateTime> = fires("~ * * * *", local_ms(2026, 1, 14, 0), 48)
        .into_iter()
        .map(|fire| Window::Hour.start_of(fire))
        .collect();
    for pair in hours.windows(2) {
        assert_eq!(pair[1] - pair[0], TimeDelta::hours(1), "{hours:?}");
    }
}

#[test]
fn re_arming_inside_a_window_keeps_its_draw() {
    let from = local_ms(2026, 1, 14, 3);
    let fire = CronScheduler
        .next_fire_ms(APP, "~ 3 * * *", from)
        .expect("a random minute yields an occurrence");
    let mut asked = from;
    while asked < fire {
        assert_eq!(
            CronScheduler.next_fire_ms(APP, "~ 3 * * *", asked),
            Some(fire)
        );
        asked += MINUTE_MS;
    }
}

#[test]
fn a_fired_window_never_fires_again() {
    let days = fires("~ 3 * * *", local_ms(2026, 1, 14, 0), 20);
    for pair in days.windows(2) {
        assert_eq!(
            pair[1].date() - pair[0].date(),
            TimeDelta::days(1),
            "{days:?}"
        );
        assert_eq!(pair[1].hour(), 3);
    }
}

#[test]
fn a_random_minute_twice_a_day_fires_exactly_twice() {
    let runs = fires("~ 8,20 * * *", local_ms(2026, 1, 14, 0), 20);
    for pair in runs.windows(2) {
        assert_ne!(pair[0].hour(), pair[1].hour(), "{runs:?}");
        assert!(pair[1] - pair[0] < TimeDelta::hours(13), "{runs:?}");
    }
}

#[test]
fn a_random_hour_fires_once_each_day() {
    let days = fires("0 ~ * * *", local_ms(2026, 1, 14, 0), 20);
    for pair in days.windows(2) {
        assert_eq!(
            pair[1].date() - pair[0].date(),
            TimeDelta::days(1),
            "{days:?}"
        );
    }
}

#[test]
fn a_random_weekday_fires_once_each_week() {
    let weeks = fires("0 0 * * ~", local_ms(2026, 1, 14, 0), 12);
    for pair in weeks.windows(2) {
        assert_eq!(
            sunday_of(pair[1]) - sunday_of(pair[0]),
            TimeDelta::days(7),
            "{weeks:?}"
        );
    }
}

#[test]
fn a_random_day_fires_at_most_once_each_month() {
    let months = fires("0 0 ~ * *", local_ms(2026, 1, 14, 0), 12);
    for pair in months.windows(2) {
        assert!(
            (pair[1].year(), pair[1].month()) > (pair[0].year(), pair[0].month()),
            "{months:?}"
        );
    }
}

#[test]
fn a_random_month_fires_once_each_year() {
    let years = fires("0 0 1 ~ *", local_ms(2026, 1, 14, 0), 6);
    for pair in years.windows(2) {
        assert_eq!(pair[1].year() - pair[0].year(), 1, "{years:?}");
    }
}

#[test]
fn services_sharing_a_schedule_draw_apart() {
    let from = local_ms(2026, 1, 14, 0);
    let seen: std::collections::BTreeSet<Option<u64>> = (0..32)
        .map(|index| CronScheduler.next_fire_ms(&format!("app{index}"), "~ 3 * * *", from))
        .collect();
    assert!(seen.len() > 1, "a tilde must spread services apart");
}
