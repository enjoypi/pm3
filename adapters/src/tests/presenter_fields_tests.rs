use super::*;

#[test]
fn a_running_process_shows_its_pid() {
    assert_eq!(format_pid(Some(4242)), "4242");
}

#[test]
fn an_idle_process_shows_no_pid() {
    assert_eq!(format_pid(None), MISSING);
}

#[test]
fn an_idle_process_shows_no_uptime() {
    assert_eq!(format_uptime(None), MISSING);
}

#[test]
fn a_sub_second_uptime_is_reported_in_milliseconds() {
    assert_eq!(format_uptime(Some(999)), "999ms");
}

#[test]
fn a_sub_minute_uptime_is_reported_in_seconds() {
    assert_eq!(format_uptime(Some(59_000)), "59s");
}

#[test]
fn a_sub_hour_uptime_is_reported_in_minutes() {
    assert_eq!(format_uptime(Some(59 * 60 * 1000)), "59m");
}

#[test]
fn a_sub_day_uptime_is_reported_in_hours() {
    assert_eq!(format_uptime(Some(23 * 60 * 60 * 1000)), "23h");
}

#[test]
fn a_longer_uptime_is_reported_in_days() {
    assert_eq!(format_uptime(Some(3 * 24 * 60 * 60 * 1000)), "3d");
}

#[test]
fn a_confined_sandbox_shows_only_its_mode() {
    assert_eq!(format_sandbox("read-only", false), "read-only");
}

#[test]
fn an_open_network_is_marked_on_the_sandbox() {
    assert_eq!(format_sandbox("read-only", true), "read-only+net");
}

#[test]
fn an_empty_list_is_shown_as_missing() {
    assert_eq!(format_list(&[]), MISSING);
}

#[test]
fn a_list_is_joined_with_commas() {
    let items = vec!["a".to_string(), "b".to_string()];
    assert_eq!(format_list(&items), "a, b");
}

#[test]
fn a_short_cell_is_padded_to_the_column_width() {
    assert_eq!(pad("ab", 4), "ab  ");
}

#[test]
fn an_oversized_cell_keeps_its_own_width() {
    assert_eq!(pad("abcdef", 2), "abcdef");
}

#[test]
fn the_widest_cell_sets_the_column_width() {
    assert_eq!(widest([1, 7, 3].into_iter()), 7);
}

#[test]
fn an_absent_column_has_no_width() {
    assert_eq!(widest(std::iter::empty()), 0);
}

#[test]
fn a_clock_without_an_instant_is_missing() {
    assert_eq!(format_clock(None), MISSING);
}

#[test]
fn a_stamp_without_an_instant_is_missing() {
    assert_eq!(format_stamp(None), MISSING);
}

#[test]
fn an_instant_beyond_the_signed_range_is_missing() {
    assert_eq!(format_clock(Some(u64::MAX)), MISSING);
}

#[test]
fn an_instant_outside_the_calendar_is_missing() {
    assert_eq!(format_stamp(Some(u64::MAX / 2)), MISSING);
}

#[test]
fn a_stamp_names_its_timezone_offset() {
    let stamp = format_stamp(Some(1_700_000_000_000));
    assert!(
        stamp.contains("UTC+") || stamp.contains("UTC-"),
        "got: {stamp}"
    );
}
