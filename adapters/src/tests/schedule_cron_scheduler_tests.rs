use usecases::Scheduler as _;

use super::*;

const MINUTE_MS: u64 = 60_000;
const SOME_INSTANT_MS: u64 = 1_700_000_000_000;

#[test]
fn the_next_occurrence_lands_within_the_coming_minute() {
    let next = CronScheduler
        .next_fire_ms("* * * * *", SOME_INSTANT_MS)
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
        .next_fire_ms("~ * * * *", SOME_INSTANT_MS)
        .expect("a random minute still yields an occurrence");
    assert!(next > SOME_INSTANT_MS);
}

#[test]
fn re_asking_a_random_schedule_eventually_moves_the_target() {
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..64 {
        seen.insert(CronScheduler.next_fire_ms("~ * * * *", SOME_INSTANT_MS));
    }
    assert!(seen.len() > 1, "a tilde must re-roll on every call");
}

#[test]
fn an_unexpandable_schedule_has_no_next_occurrence() {
    assert_eq!(
        CronScheduler.next_fire_ms("0~59/0 * * * *", SOME_INSTANT_MS),
        None
    );
}

#[test]
fn an_unparsable_schedule_has_no_next_occurrence() {
    assert_eq!(
        CronScheduler.next_fire_ms("nonsense", SOME_INSTANT_MS),
        None
    );
}

#[test]
fn a_schedule_that_never_matches_has_no_next_occurrence() {
    assert_eq!(
        CronScheduler.next_fire_ms("0 0 30 2 *", SOME_INSTANT_MS),
        None
    );
}

#[test]
fn a_timestamp_beyond_the_signed_range_has_no_next_occurrence() {
    assert_eq!(CronScheduler.next_fire_ms("* * * * *", u64::MAX), None);
}

#[test]
fn a_timestamp_outside_the_calendar_has_no_next_occurrence() {
    let beyond_chrono = u64::MAX / 2;
    assert_eq!(CronScheduler.next_fire_ms("* * * * *", beyond_chrono), None);
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
