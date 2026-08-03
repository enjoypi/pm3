use super::*;

fn seeded(seed: u64) -> fastrand::Rng {
    fastrand::Rng::with_seed(seed)
}

fn expanded(pattern: &str, seed: u64) -> String {
    let mut rng = seeded(seed);
    expand_random(pattern, &mut rng).expect("pattern should expand")
}

fn first_field(pattern: &str, seed: u64) -> String {
    expanded(pattern, seed)
        .split_whitespace()
        .next()
        .expect("an expanded pattern keeps its fields")
        .to_string()
}

fn minute_of(pattern: &str, seed: u64) -> u32 {
    first_field(pattern, seed)
        .parse()
        .expect("a bounded tilde expands to a bare number")
}

fn weekday_of(pattern: &str, seed: u64) -> String {
    expanded(pattern, seed)
        .split_whitespace()
        .nth(4)
        .expect("weekday field present")
        .to_string()
}

#[test]
fn a_pattern_without_a_tilde_is_left_alone() {
    assert_eq!(expanded("30 9,18 * * *", 7), "30 9,18 * * *");
}

#[test]
fn a_bare_tilde_picks_a_value_from_the_whole_field() {
    let minute = minute_of("~ * * * *", 7);
    assert!(minute <= 59, "minute out of range: {minute}");
}

#[test]
fn a_bare_tilde_in_the_hour_field_respects_that_field_range() {
    let expansion = expanded("0 ~ * * *", 11);
    let hour: u32 = expansion
        .split_whitespace()
        .nth(1)
        .expect("hour field present")
        .parse()
        .expect("hour expands to a bare number");
    assert!(hour <= 23, "hour out of range: {hour}");
}

#[test]
fn a_bounded_tilde_stays_within_its_bounds() {
    for seed in 0..32 {
        let minute = minute_of("25~35 9,18 * * *", seed);
        assert!(
            (25..=35).contains(&minute),
            "minute out of bounds: {minute}"
        );
    }
}

#[test]
fn a_bounded_tilde_leaves_the_other_fields_untouched() {
    let expansion = expanded("25~35 9,18 * * *", 3);
    let (_minutes, tail) = expansion.split_once(' ').expect("more than one field");
    assert_eq!(tail, "9,18 * * *");
}

#[test]
fn a_tilde_with_a_step_renders_a_stepped_range() {
    let field = first_field("0~59/10 * * * *", 5);
    let (offset, rest) = field.split_once('-').expect("a stepped range keeps a dash");
    assert_eq!(rest, "59/10");
    let offset: u32 = offset.parse().expect("offset is numeric");
    assert!(offset < 10, "offset must fall inside one step: {offset}");
}

#[test]
fn the_same_seed_expands_identically() {
    assert_eq!(expanded("~ * * * *", 42), expanded("~ * * * *", 42));
}

#[test]
fn re_rolling_eventually_yields_a_different_value() {
    let mut rng = seeded(1);
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..64 {
        seen.insert(expand_random("~ * * * *", &mut rng).expect("expands"));
    }
    assert!(seen.len() > 1, "a tilde must re-roll across calls");
}

#[test]
fn a_field_count_other_than_five_is_passed_through_untouched() {
    assert_eq!(expanded("0 0 * * * *", 1), "0 0 * * * *");
}

#[test]
fn a_zero_step_is_rejected() {
    let mut rng = seeded(1);
    let err = expand_random("0~59/0 * * * *", &mut rng).unwrap_err();
    assert_eq!(
        err,
        ExpandError::ZeroStep {
            field: "0~59/0".to_string()
        }
    );
}

#[test]
fn an_inverted_range_is_rejected() {
    let mut rng = seeded(1);
    let err = expand_random("35~25 * * * *", &mut rng).unwrap_err();
    assert_eq!(
        err,
        ExpandError::InvertedRange {
            field: "35~25".to_string()
        }
    );
}

#[test]
fn a_bound_outside_the_field_range_is_rejected() {
    let mut rng = seeded(1);
    let err = expand_random("0~99 * * * *", &mut rng).unwrap_err();
    assert_eq!(
        err,
        ExpandError::OutOfRange {
            field: "0~99".to_string(),
            low: 0,
            high: 59,
        }
    );
}

#[test]
fn a_non_numeric_bound_is_rejected() {
    let mut rng = seeded(1);
    let err = expand_random("a~b * * * *", &mut rng).unwrap_err();
    assert_eq!(
        err,
        ExpandError::Malformed {
            field: "a~b".to_string()
        }
    );
}

#[test]
fn a_non_numeric_step_is_rejected() {
    let mut rng = seeded(1);
    let err = expand_random("0~59/x * * * *", &mut rng).unwrap_err();
    assert_eq!(
        err,
        ExpandError::Malformed {
            field: "0~59/x".to_string()
        }
    );
}

#[test]
fn every_expand_error_renders_a_message() {
    let errors = [
        ExpandError::ZeroStep {
            field: "0~59/0".to_string(),
        },
        ExpandError::InvertedRange {
            field: "35~25".to_string(),
        },
        ExpandError::OutOfRange {
            field: "0~99".to_string(),
            low: 0,
            high: 59,
        },
        ExpandError::Malformed {
            field: "a~b".to_string(),
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
fn an_open_low_bound_falls_back_to_the_field_floor() {
    for seed in 0..32 {
        let minute = minute_of("~30 * * * *", seed);
        assert!(minute <= 30, "minute should stay under the cap: {minute}");
    }
}

#[test]
fn an_open_high_bound_falls_back_to_the_field_ceiling() {
    for seed in 0..32 {
        let minute = minute_of("30~ * * * *", seed);
        assert!(minute >= 30, "minute should stay above the floor: {minute}");
    }
}

#[test]
fn a_bare_tilde_with_a_step_spans_the_whole_field() {
    let field = first_field("~/10 * * * *", 5);
    assert!(field.ends_with("-59/10"), "got: {field}");
}

#[test]
fn a_bound_below_the_field_floor_is_rejected() {
    let mut rng = seeded(1);
    let err = expand_random("* * 0~10 * *", &mut rng).unwrap_err();
    assert_eq!(
        err,
        ExpandError::OutOfRange {
            field: "0~10".to_string(),
            low: 1,
            high: 31,
        }
    );
}

#[test]
fn a_non_numeric_high_bound_is_rejected_on_its_own() {
    let mut rng = seeded(1);
    let err = expand_random("25~b * * * *", &mut rng).unwrap_err();
    assert_eq!(
        err,
        ExpandError::Malformed {
            field: "25~b".to_string()
        }
    );
}

#[test]
fn the_weekday_field_accepts_sunday_written_as_seven() {
    for seed in 0..32 {
        let day: u32 = weekday_of("0 0 * * 5~7", seed)
            .parse()
            .expect("a bounded tilde expands to a bare number");
        assert!(
            matches!(day, 0 | 5 | 6),
            "seven must normalise to sunday-zero: {day}"
        );
    }
}

#[test]
fn a_weekday_tilde_never_renders_a_bare_seven() {
    for seed in 0..64 {
        let day = weekday_of("0 0 * * ~", seed);
        assert_ne!(day, "7", "cron semantics render sunday as zero");
    }
}

#[test]
fn a_weekday_bound_above_seven_is_rejected() {
    let mut rng = seeded(1);
    let err = expand_random("0 0 * * 0~8", &mut rng).unwrap_err();
    assert_eq!(
        err,
        ExpandError::OutOfRange {
            field: "0~8".to_string(),
            low: 0,
            high: 7,
        }
    );
}

#[test]
fn a_stepped_weekday_range_keeps_its_ceiling() {
    let field = weekday_of("0 0 * * 0~7/3", 5);
    assert!(field.ends_with("-7/3"), "got: {field}");
}
