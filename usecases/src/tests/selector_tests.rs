use super::*;

#[test]
fn digits_parse_as_a_process_id() {
    assert_eq!(AppSelector::parse("7"), AppSelector::Id(7));
}

#[test]
fn non_digits_parse_as_a_name() {
    assert_eq!(
        AppSelector::parse("api"),
        AppSelector::Name("api".to_string())
    );
}

#[test]
fn an_all_digit_name_beyond_u32_parses_as_a_name() {
    let huge = "99999999999999999999";
    assert_eq!(
        AppSelector::parse(huge),
        AppSelector::Name(huge.to_string())
    );
}

#[test]
fn id_selector_matches_by_process_id_only() {
    let selector = AppSelector::Id(7);
    assert!(selector.matches(7, "api"));
    assert!(!selector.matches(8, "api"));
}

#[test]
fn name_selector_matches_by_name_only() {
    let selector = AppSelector::Name("api".to_string());
    assert!(selector.matches(7, "api"));
    assert!(!selector.matches(7, "web"));
}

#[test]
fn id_selector_renders_the_number() {
    assert_eq!(AppSelector::Id(7).to_string(), "7");
}

#[test]
fn name_selector_renders_the_name() {
    assert_eq!(AppSelector::Name("api".to_string()).to_string(), "api");
}
