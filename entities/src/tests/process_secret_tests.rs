use super::*;

#[test]
fn a_long_value_shows_only_its_ends_and_its_size() {
    let masked = mask_secret("abcdefghijklmnop");
    assert_eq!(masked, "abcd..mnop 16", "got: {masked}");
}

#[test]
fn a_value_at_the_threshold_still_shows_its_ends() {
    let value = "0123456789ab";
    assert_eq!(
        value.chars().count(),
        MIN_MASKABLE_CHARS,
        "the fixture pins the edge"
    );
    let masked = mask_secret(value);
    assert_eq!(masked, "0123..89ab 12", "got: {masked}");
}

#[test]
fn a_value_below_the_threshold_shows_nothing_but_its_size() {
    let value = "0123456789a";
    assert_eq!(
        value.chars().count(),
        MIN_MASKABLE_CHARS - 1,
        "the fixture pins the edge"
    );
    let masked = mask_secret(value);
    assert_eq!(masked, ".. 11", "got: {masked}");
}

#[test]
fn a_short_value_shows_nothing_but_its_size() {
    assert_eq!(mask_secret("8080"), ".. 4");
}

#[test]
fn an_empty_value_still_reports_its_size() {
    assert_eq!(mask_secret(""), ".. 0");
}

#[test]
fn a_multibyte_value_counts_characters_not_bytes() {
    let masked = mask_secret("一二三四五六七八九十甲乙");
    assert_eq!(
        masked, "一二三四..九十甲乙 12",
        "the ends and the size share one unit, got: {masked}"
    );
}

#[test]
fn a_multibyte_value_with_too_few_characters_shows_nothing() {
    let masked = mask_secret("一二三四五六七八");
    assert_eq!(
        masked, ".. 8",
        "showing four characters from each end would hand over the whole value, got: {masked}"
    );
}
