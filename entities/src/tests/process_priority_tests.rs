use super::*;

#[test]
fn every_documented_priority_parses() {
    assert_eq!(Priority::parse("background"), Some(Priority::Background));
    assert_eq!(Priority::parse("standard"), Some(Priority::Standard));
    assert_eq!(Priority::parse("interactive"), Some(Priority::Interactive));
}

#[test]
fn an_unknown_priority_is_rejected() {
    assert_eq!(Priority::parse("realtime"), None);
}

#[test]
fn a_service_runs_in_the_background_unless_it_says_otherwise() {
    assert_eq!(Priority::default(), Priority::Background);
}
