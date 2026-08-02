#![allow(
    clippy::literal_string_with_formatting_args,
    reason = "${VAR:-default} in test inputs are parser fixtures, not format args"
)]

use std::ffi::OsString;

use super::{test_helpers::*, *};

#[test]
fn substitute_uses_looked_up_value_when_present() {
    let result = substitute_fake("host: ${TEST_SKEL_SET:-0.0.0.0}");
    assert_eq!(result, "host: 192.168.1.1");
}

#[test]
fn substitute_uses_default_when_absent() {
    let result = substitute_fake("host: ${TEST_SKEL_UNSET_VAR:-localhost}");
    assert_eq!(result, "host: localhost");
}

#[test]
fn substitute_fails_when_absent_without_default() {
    let err = substitute_with("host: ${TEST_SKEL_NODEFAULT}", fake_lookup).expect_err(
        "a placeholder without default must fail loudly instead of yielding an empty secret",
    );
    assert_eq!(
        err.to_string(),
        "cannot resolve environment variable 'TEST_SKEL_NODEFAULT': not set and placeholder declares no default"
    );
}

#[test]
fn substitute_uses_looked_up_value_without_default() {
    let result = substitute_fake("host: ${TEST_SKEL_SET}");
    assert_eq!(result, "host: 192.168.1.1");
}

#[test]
fn substitute_propagates_lookup_error() {
    let err = substitute_with("v: ${TEST_SKEL_NOT_UNICODE:-fallback}", fake_lookup)
        .expect_err("non-UTF-8 env var must fail loudly instead of silently using the default");
    assert!(
        matches!(err, ConfigLoadError::EnvVarNotUnicode { ref name } if name == NOT_UNICODE_VAR),
        "got: {err}"
    );
}

#[test]
fn substitute_rejects_env_value_containing_double_quote() {
    let err = substitute_with(r#"url: "${TEST_SKEL_QUOTE}""#, fake_lookup).expect_err(
        "a value with a double quote must be rejected instead of breaking the YAML document",
    );
    assert_eq!(
        err.to_string(),
        "cannot substitute environment variable 'TEST_SKEL_QUOTE': value contains '\\\"', which would change the YAML document structure"
    );
}

#[test]
fn substitute_rejects_env_value_containing_newline() {
    let err = substitute_with("url: ${TEST_SKEL_NEWLINE}", fake_lookup)
        .expect_err("a value with a newline must not be able to inject sibling config keys");
    assert!(
        matches!(err, ConfigLoadError::EnvVarNotYamlSafe { ref name, .. } if name == NEWLINE_VAR),
        "got: {err}"
    );
}

#[test]
fn substitute_rejects_env_value_containing_backslash() {
    let err = substitute_with(r#"path: "${TEST_SKEL_BACKSLASH}""#, fake_lookup)
        .expect_err("a backslash is an escape introducer inside a quoted YAML scalar");
    assert!(
        matches!(err, ConfigLoadError::EnvVarNotYamlSafe { ref name, .. } if name == BACKSLASH_VAR),
        "got: {err}"
    );
}

#[test]
fn substitute_no_placeholders() {
    let input = "host: localhost\nport: 9229";
    let result = substitute_fake(input);
    assert_eq!(result, input);
}

#[test]
fn substitute_default_closes_at_first_brace() {
    let result = substitute_fake("password: ${TEST_SKEL_BRACE_PWD:-pa}ss}");
    assert_eq!(result, "password: pass}");
}

#[test]
fn substitute_closes_at_first_brace_leaving_trailing_text() {
    let result = substitute_fake("v: ${TEST_SKEL_SET}ss}");
    assert_eq!(result, "v: 192.168.1.1ss}");
}

#[test]
fn substitute_keeps_braced_default_intact_when_value_is_found() {
    let result = substitute_fake(r#"v: ${TEST_SKEL_SET:-{"k":1}}"#);
    assert_eq!(result, "v: 192.168.1.1");
}

#[test]
fn substitute_keeps_braced_default_intact_when_value_is_absent() {
    let result = substitute_fake(r#"v: ${TEST_SKEL_UNSET:-{"k":1}}"#);
    assert_eq!(result, r#"v: {"k":1}"#);
}

#[test]
fn substitute_nested_placeholder_default_is_consumed_whole() {
    let result = substitute_fake("v: ${TEST_SKEL_SET:-${OTHER}}");
    assert_eq!(result, "v: 192.168.1.1");
}

#[test]
fn substitute_inside_yaml_flow_mapping_keeps_closing_brace() {
    let result = substitute_fake(r#"extra: {u: "${TEST_SKEL_FLOW:-d}"}"#);
    assert_eq!(result, r#"extra: {u: "d"}"#);
}

#[test]
fn substitute_inside_yaml_flow_mapping_resolves_required_placeholder() {
    let result = substitute_fake(r#"extra: {u: "${TEST_SKEL_SET}"}"#);
    assert_eq!(result, r#"extra: {u: "192.168.1.1"}"#);
}

#[test]
fn substitute_required_placeholder_before_trailing_brace_still_fails_loudly() {
    let err = substitute_with(r#"extra: {u: "${TEST_SKEL_NODEFAULT}"}"#, fake_lookup)
        .expect_err("a trailing brace must not turn a required placeholder into a literal");
    assert!(
        matches!(err, ConfigLoadError::EnvVarNotSet { ref name } if name == "TEST_SKEL_NODEFAULT"),
        "got: {err}"
    );
}

#[test]
fn substitute_two_placeholders_on_same_line_do_not_swallow_each_other() {
    let result = substitute_fake("k: ${TEST_SKEL_TWO_A:-aa}-${TEST_SKEL_TWO_B:-bb}");
    assert_eq!(result, "k: aa-bb");
}

#[test]
fn substitute_unclosed_placeholder_preserved_verbatim() {
    let result = substitute_fake("v: ${UNCLOSED");
    assert_eq!(result, "v: ${UNCLOSED");
}

#[test]
fn substitute_empty_name_preserved_verbatim() {
    let result = substitute_fake("v: ${}");
    assert_eq!(result, "v: ${}");
}

#[test]
fn substitute_name_with_colon_preserved_verbatim() {
    let result = substitute_fake("v: ${A:B:-x}");
    assert_eq!(result, "v: ${A:B:-x}");
}

#[test]
fn substitute_name_with_open_brace_preserved_verbatim() {
    let result = substitute_fake("v: ${A{B}");
    assert_eq!(result, "v: ${A{B}");
}

#[test]
fn substitute_name_with_balanced_braces_preserved_verbatim() {
    let result = substitute_fake("v: ${A{B}:-x}");
    assert_eq!(result, "v: ${A{B}:-x}");
}

#[test]
fn substitute_nested_placeholder_without_close_preserved_verbatim() {
    let result = substitute_fake("v: ${A${B");
    assert_eq!(result, "v: ${A${B");
}

#[test]
fn substitute_placeholder_with_empty_default() {
    let result = substitute_fake("v: ${TEST_SKEL_EMPTY_DEF:-}");
    assert_eq!(result, "v: ");
}

#[test]
fn substitute_placeholder_spans_multiline_only_first_line() {
    let result = substitute_fake("v: ${UNCLOSED\nnext: line");
    assert_eq!(result, "v: ${UNCLOSED\nnext: line");
}

#[test]
fn substitute_keeps_the_reserved_service_cwd_placeholder() {
    let result = substitute_fake("args: [\"${PM3_SVC_CWD}/data\"]");
    assert_eq!(result, "args: [\"${PM3_SVC_CWD}/data\"]");
}

#[test]
fn substitute_rejects_a_default_on_the_reserved_placeholder() {
    let err = substitute_with("args: [\"${PM3_SVC_CWD:-/fallback}\"]", fake_lookup)
        .expect_err("the reserved placeholder must not accept a default");
    assert_eq!(
        err.to_string(),
        "cannot resolve environment variable 'PM3_SVC_CWD': the reserved placeholder does not accept a ':-' default"
    );
}

#[test]
fn substitute_keeps_the_reserved_placeholder_next_to_a_resolved_one() {
    let result = substitute_fake("a: ${PM3_SVC_CWD}\nb: ${TEST_SKEL_SET}");
    assert_eq!(result, "a: ${PM3_SVC_CWD}\nb: 192.168.1.1");
}

#[test]
fn substitute_env_vars_reads_process_env() {
    let result = substitute_env_vars("host: ${TEST_SKEL_NEVER_DEFINED:-localhost}")
        .expect("absent env var must fall back to the default");
    assert_eq!(result, "host: localhost");
}

#[test]
fn classify_env_var_returns_value_when_present() {
    let classified = classify_env_var(SET_VAR, Ok(SET_VALUE.to_string()));
    assert_eq!(
        classified.expect("present value must classify as Ok"),
        Some(SET_VALUE.to_string())
    );
}

#[test]
fn classify_env_var_returns_none_when_not_present() {
    let classified = classify_env_var(SET_VAR, Err(env::VarError::NotPresent));
    assert_eq!(classified.expect("absent value must classify as Ok"), None);
}

#[test]
fn classify_env_var_returns_error_when_not_unicode() {
    let read = Err(env::VarError::NotUnicode(OsString::from("non-utf8")));
    let err = classify_env_var(NOT_UNICODE_VAR, read)
        .expect_err("non-UTF-8 env var must not silently fall back to the default");
    assert_eq!(
        err.to_string(),
        "cannot decode environment variable 'TEST_SKEL_NOT_UNICODE': contains non-UTF-8 bytes"
    );
}
