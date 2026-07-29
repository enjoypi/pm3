use super::*;

#[test]
fn validate_name_valid() {
    Example::validate_name("hello").expect("should be valid");
}

#[test]
fn validate_name_empty() {
    let err = Example::validate_name("").unwrap_err();
    assert!(err.to_string().contains("empty"), "got: {err}");
}

#[test]
fn validate_name_whitespace_only() {
    let err = Example::validate_name("   ").unwrap_err();
    assert!(err.to_string().contains("empty"), "got: {err}");
}
