use super::*;

#[test]
fn validate_rejects_an_unknown_sandbox_read_scope() {
    let mut cfg = valid_config();
    cfg.pm3.sandbox.read = "everything".to_string();
    let err = validate_config(&cfg).unwrap_err();
    assert!(
        matches!(err, ConfigError::InvalidSandboxRead { ref read, .. } if read == "everything"),
        "got: {err}"
    );
}

#[test]
fn validate_rejects_an_empty_minimal_read_allowlist() {
    let mut cfg = valid_config();
    cfg.pm3.sandbox.minimal_read_roots = Vec::new();
    let err = validate_config(&cfg).unwrap_err();
    assert!(
        matches!(err, ConfigError::EmptyMinimalReadRoots),
        "got: {err}"
    );
}

#[test]
fn validate_rejects_a_relative_minimal_read_root() {
    let mut cfg = valid_config();
    cfg.pm3.sandbox.minimal_read_roots = vec!["usr".to_string()];
    let err = validate_config(&cfg).unwrap_err();
    assert!(
        matches!(
            err,
            ConfigError::RelativeSandboxRoot { field, ref root }
                if field == "pm3.sandbox.minimal_read_roots" && root == "usr"
        ),
        "got: {err}"
    );
}

#[test]
fn validate_rejects_a_relative_forbidden_writable_root() {
    let mut cfg = valid_config();
    cfg.pm3.sandbox.forbidden_writable_roots = vec!["etc".to_string()];
    let err = validate_config(&cfg).unwrap_err();
    assert!(
        matches!(
            err,
            ConfigError::RelativeSandboxRoot { field, .. }
                if field == "pm3.sandbox.forbidden_writable_roots"
        ),
        "got: {err}"
    );
}

#[test]
fn validate_accepts_an_empty_forbidden_writable_root_list() {
    let mut cfg = valid_config();
    cfg.pm3.sandbox.forbidden_writable_roots = Vec::new();
    validate_config(&cfg).expect("an operator may take the guard rails off");
}
