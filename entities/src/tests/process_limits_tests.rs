use super::*;

#[test]
fn a_bare_number_counts_bytes() {
    assert_eq!(parse_memory_limit("2048"), Some(2));
}

#[test]
fn a_byte_suffix_counts_bytes() {
    assert_eq!(parse_memory_limit("4096B"), Some(4));
}

#[test]
fn a_kibibyte_suffix_is_taken_as_written() {
    assert_eq!(parse_memory_limit("512K"), Some(512));
    assert_eq!(parse_memory_limit("512kb"), Some(512));
    assert_eq!(parse_memory_limit("512KiB"), Some(512));
}

#[test]
fn a_mebibyte_suffix_scales_by_a_thousand_and_twenty_four() {
    assert_eq!(parse_memory_limit("300M"), Some(300 * 1024));
}

#[test]
fn a_gibibyte_suffix_scales_twice() {
    assert_eq!(parse_memory_limit("2G"), Some(2 * 1024 * 1024));
}

#[test]
fn surrounding_and_inner_whitespace_is_tolerated() {
    assert_eq!(parse_memory_limit("  300 M "), Some(300 * 1024));
}

#[test]
fn an_unknown_unit_is_refused() {
    assert_eq!(parse_memory_limit("300T"), None);
}

#[test]
fn a_missing_amount_is_refused() {
    assert_eq!(parse_memory_limit("M"), None);
}

#[test]
fn an_empty_limit_is_refused() {
    assert_eq!(parse_memory_limit(""), None);
}

#[test]
fn an_amount_below_a_kibibyte_is_refused() {
    assert_eq!(parse_memory_limit("512"), None);
}

#[test]
fn an_amount_that_overflows_is_refused() {
    assert_eq!(parse_memory_limit("18446744073709551615G"), None);
}

#[test]
fn no_limit_never_breaches() {
    assert_eq!(decide_memory_verdict(None, u64::MAX), MemoryVerdict::Within);
}

#[test]
fn usage_below_the_limit_stays_within() {
    assert_eq!(
        decide_memory_verdict(Some(1000), 999),
        MemoryVerdict::Within
    );
}

#[test]
fn usage_at_the_limit_stays_within() {
    assert_eq!(
        decide_memory_verdict(Some(1000), 1000),
        MemoryVerdict::Within
    );
}

#[test]
fn usage_above_the_limit_breaches() {
    assert_eq!(
        decide_memory_verdict(Some(1000), 1001),
        MemoryVerdict::Breached
    );
}

#[test]
fn only_a_breach_reports_itself_as_one() {
    assert!(MemoryVerdict::Breached.is_breached());
    assert!(!MemoryVerdict::Within.is_breached());
}
