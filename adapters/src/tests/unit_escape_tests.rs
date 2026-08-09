use super::*;

#[test]
fn plain_text_passes_through_untouched() {
    assert_eq!(escape_xml("plain text 123"), "plain text 123");
}

#[test]
fn every_markup_character_is_escaped() {
    assert_eq!(
        escape_xml("a&b<c>d\"e'f"),
        "a&amp;b&lt;c&gt;d&quot;e&apos;f"
    );
}

#[test]
fn an_empty_string_stays_empty() {
    assert_eq!(escape_xml(""), "");
}
