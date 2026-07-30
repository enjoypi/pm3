use super::*;

fn workspace_policy() -> SandboxPolicy {
    SandboxPolicy {
        mode: SandboxMode::WorkspaceWrite,
        network: false,
        writable_roots: vec!["/srv/api".to_string()],
        derived_roots: Vec::new(),
    }
}

#[test]
fn parse_round_trips_every_mode() {
    for mode in [
        SandboxMode::ReadOnly,
        SandboxMode::WorkspaceWrite,
        SandboxMode::DangerFullAccess,
    ] {
        assert_eq!(SandboxMode::parse(mode.as_str()), Some(mode));
    }
}

#[test]
fn parse_rejects_unknown_mode() {
    assert_eq!(SandboxMode::parse("yolo"), None);
}

#[test]
fn read_only_denies_writes() {
    assert!(!SandboxMode::ReadOnly.allows_writes());
}

#[test]
fn workspace_write_allows_writes() {
    assert!(SandboxMode::WorkspaceWrite.allows_writes());
}

#[test]
fn danger_full_access_allows_writes() {
    assert!(SandboxMode::DangerFullAccess.allows_writes());
}

#[test]
fn danger_full_access_is_unconfined() {
    assert!(SandboxMode::DangerFullAccess.is_unconfined());
}

#[test]
fn workspace_write_stays_confined() {
    assert!(!SandboxMode::WorkspaceWrite.is_unconfined());
}

#[test]
fn read_only_stays_confined() {
    assert!(!SandboxMode::ReadOnly.is_unconfined());
}

#[test]
fn validate_accepts_workspace_write_with_absolute_root() {
    validate_policy(&workspace_policy()).expect("absolute writable root is valid");
}

#[test]
fn validate_accepts_read_only_without_writable_roots() {
    let policy = SandboxPolicy {
        mode: SandboxMode::ReadOnly,
        network: false,
        writable_roots: Vec::new(),
        derived_roots: Vec::new(),
    };
    validate_policy(&policy).expect("read-only without writable roots is valid");
}

#[test]
fn validate_rejects_empty_writable_root() {
    let policy = SandboxPolicy {
        writable_roots: vec![String::new()],
        ..workspace_policy()
    };
    let err = validate_policy(&policy).unwrap_err();
    assert!(matches!(err, PolicyError::EmptyWritableRoot), "got: {err}");
}

#[test]
fn validate_rejects_relative_writable_root() {
    let policy = SandboxPolicy {
        writable_roots: vec!["srv/api".to_string()],
        ..workspace_policy()
    };
    let err = validate_policy(&policy).unwrap_err();
    assert!(
        matches!(err, PolicyError::RelativeWritableRoot(ref root) if root == "srv/api"),
        "got: {err}"
    );
}

#[test]
fn validate_rejects_writable_roots_under_read_only_mode() {
    let policy = SandboxPolicy {
        mode: SandboxMode::ReadOnly,
        ..workspace_policy()
    };
    let err = validate_policy(&policy).unwrap_err();
    assert!(
        matches!(err, PolicyError::WritableRootsWithoutWriteAccess),
        "got: {err}"
    );
}

#[test]
fn every_policy_error_renders_a_message() {
    let errors = [
        PolicyError::EmptyWritableRoot,
        PolicyError::RelativeWritableRoot("srv".to_string()),
        PolicyError::WritableRootsWithoutWriteAccess,
    ];
    for err in errors {
        assert!(
            err.to_string().starts_with("cannot accept"),
            "error message must start with a verb: {err}"
        );
    }
}
