use super::*;

pub const SET_VAR: &str = "TEST_SKEL_SET";
pub const NOT_UNICODE_VAR: &str = "TEST_SKEL_NOT_UNICODE";
pub const SET_VALUE: &str = "192.168.1.1";
pub const QUOTE_VAR: &str = "TEST_SKEL_QUOTE";
pub const NEWLINE_VAR: &str = "TEST_SKEL_NEWLINE";
pub const BACKSLASH_VAR: &str = "TEST_SKEL_BACKSLASH";

pub fn fake_lookup(name: &str) -> Result<Option<String>, ConfigLoadError> {
    match name {
        SET_VAR => Ok(Some(SET_VALUE.to_string())),
        QUOTE_VAR => Ok(Some("pa\"ss".to_string())),
        NEWLINE_VAR => Ok(Some("host\nport: 1".to_string())),
        BACKSLASH_VAR => Ok(Some("C:\\data".to_string())),
        NOT_UNICODE_VAR => Err(ConfigLoadError::EnvVarNotUnicode {
            name: name.to_string(),
        }),
        _ => Ok(None),
    }
}

pub fn substitute_fake(raw: &str) -> String {
    substitute_with(raw, fake_lookup)
        .expect("env substitution should not fail in this test fixture")
}
