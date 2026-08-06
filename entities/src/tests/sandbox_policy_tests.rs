use super::*;

fn workspace_policy() -> SandboxPolicy {
    SandboxPolicy {
        mode: SandboxMode::WorkspaceWrite,
        read: ReadScope::Minimal,
        network: false,
        writable_roots: vec!["/srv/api".to_string()],
        readable_roots: Vec::new(),
        derived_roots: Vec::new(),
        unreadable_roots: Vec::new(),
    }
}

#[test]
fn parse_round_trips_every_mode() {
    for mode in SandboxMode::ALL {
        assert_eq!(SandboxMode::parse(mode.as_str()), Some(mode));
    }
}

#[test]
fn parse_rejects_unknown_mode() {
    assert_eq!(SandboxMode::parse("yolo"), None);
}

#[test]
fn parse_round_trips_every_read_scope() {
    for scope in ReadScope::ALL {
        assert_eq!(ReadScope::parse(scope.as_str()), Some(scope));
    }
}

#[test]
fn parse_rejects_unknown_read_scope() {
    assert_eq!(ReadScope::parse("everything"), None);
}

#[test]
fn minimal_read_scope_confines_reads() {
    assert!(ReadScope::Minimal.confines_reads());
}

#[test]
fn full_read_scope_does_not_confine_reads() {
    assert!(!ReadScope::Full.confines_reads());
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
fn readable_paths_cover_declared_reads_and_every_writable_root() {
    let policy = SandboxPolicy {
        readable_roots: vec!["/opt/data".to_string()],
        derived_roots: vec!["/home/me/.pm3/api".to_string()],
        ..workspace_policy()
    };
    assert_eq!(
        policy.readable_paths(),
        vec!["/opt/data", "/srv/api", "/home/me/.pm3/api"]
    );
}

#[test]
fn hidden_paths_expose_the_unreadable_roots() {
    let policy = SandboxPolicy {
        unreadable_roots: vec!["/home/me/.config/pm3".to_string()],
        ..workspace_policy()
    };
    assert_eq!(policy.hidden_paths(), vec!["/home/me/.config/pm3"]);
}

#[test]
fn validate_accepts_workspace_write_with_absolute_root() {
    validate_policy(&workspace_policy()).expect("absolute writable root is valid");
}

#[test]
fn validate_accepts_read_only_without_writable_roots() {
    let policy = SandboxPolicy {
        mode: SandboxMode::ReadOnly,
        read: ReadScope::Minimal,
        network: false,
        writable_roots: Vec::new(),
        readable_roots: Vec::new(),
        derived_roots: Vec::new(),
        unreadable_roots: Vec::new(),
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
fn validate_rejects_empty_derived_root() {
    let policy = SandboxPolicy {
        derived_roots: vec![String::new()],
        ..workspace_policy()
    };
    let err = validate_policy(&policy).unwrap_err();
    assert!(matches!(err, PolicyError::EmptyDerivedRoot), "got: {err}");
}

#[test]
fn validate_rejects_relative_derived_root_without_blaming_the_declaration() {
    let policy = SandboxPolicy {
        derived_roots: vec!["var/log".to_string()],
        ..workspace_policy()
    };
    let err = validate_policy(&policy).unwrap_err();
    assert!(
        matches!(err, PolicyError::RelativeDerivedRoot(ref root) if root == "var/log"),
        "got: {err}"
    );
    assert!(err.to_string().contains("derived"), "got: {err}");
}

#[test]
fn validate_rejects_empty_readable_root() {
    let policy = SandboxPolicy {
        readable_roots: vec![String::new()],
        ..workspace_policy()
    };
    let err = validate_policy(&policy).unwrap_err();
    assert!(matches!(err, PolicyError::EmptyReadableRoot), "got: {err}");
}

#[test]
fn validate_rejects_relative_readable_root() {
    let policy = SandboxPolicy {
        readable_roots: vec!["opt/data".to_string()],
        ..workspace_policy()
    };
    let err = validate_policy(&policy).unwrap_err();
    assert!(
        matches!(err, PolicyError::RelativeReadableRoot(ref root) if root == "opt/data"),
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
fn validate_rejects_a_writable_root_that_would_reopen_a_hidden_root() {
    let policy = SandboxPolicy {
        writable_roots: vec!["/home/me".to_string()],
        unreadable_roots: vec!["/home/me/.config/pm3".to_string()],
        ..workspace_policy()
    };
    let err = validate_policy(&policy).unwrap_err();
    assert!(
        matches!(
            err,
            PolicyError::WritableRootCoversHiddenRoot { ref root, ref hidden }
                if root == "/home/me" && hidden == "/home/me/.config/pm3"
        ),
        "got: {err}"
    );
}

#[test]
fn validate_accepts_a_writable_root_that_merely_shares_a_prefix_with_a_hidden_root() {
    let policy = SandboxPolicy {
        writable_roots: vec!["/home/mel".to_string()],
        unreadable_roots: vec!["/home/me/.config/pm3".to_string()],
        ..workspace_policy()
    };
    validate_policy(&policy).expect("a sibling path is not a parent");
}

#[test]
fn forbidden_roots_reject_a_declared_writable_root() {
    let policy = SandboxPolicy {
        writable_roots: vec!["/etc".to_string()],
        ..workspace_policy()
    };
    let forbidden = vec!["/".to_string(), "/etc".to_string()];
    let err = validate_forbidden_roots(&policy, &forbidden).unwrap_err();
    assert!(
        matches!(err, PolicyError::ForbiddenWritableRoot(ref root) if root == "/etc"),
        "got: {err}"
    );
}

#[test]
fn forbidden_roots_reject_a_trailing_slash_spelling() {
    let policy = SandboxPolicy {
        writable_roots: vec!["/etc/".to_string()],
        ..workspace_policy()
    };
    let forbidden = vec!["/etc".to_string()];
    let err = validate_forbidden_roots(&policy, &forbidden).unwrap_err();
    assert!(
        matches!(err, PolicyError::ForbiddenWritableRoot(ref root) if root == "/etc/"),
        "got: {err}"
    );
}

#[test]
fn forbidden_roots_accept_a_root_below_a_forbidden_one() {
    let policy = SandboxPolicy {
        writable_roots: vec!["/var/lib/api".to_string()],
        ..workspace_policy()
    };
    let forbidden = vec!["/var".to_string()];
    validate_forbidden_roots(&policy, &forbidden).expect("only the root itself is forbidden");
}

#[test]
fn forbidden_roots_ignore_the_derived_roots() {
    let policy = SandboxPolicy {
        writable_roots: Vec::new(),
        derived_roots: vec!["/var".to_string()],
        ..workspace_policy()
    };
    let forbidden = vec!["/var".to_string()];
    validate_forbidden_roots(&policy, &forbidden).expect("pm3 derives those itself");
}

#[test]
fn every_policy_error_renders_a_message() {
    let errors = [
        PolicyError::EmptyWritableRoot,
        PolicyError::RelativeWritableRoot("srv".to_string()),
        PolicyError::EmptyDerivedRoot,
        PolicyError::RelativeDerivedRoot("var/log".to_string()),
        PolicyError::EmptyReadableRoot,
        PolicyError::RelativeReadableRoot("opt".to_string()),
        PolicyError::WritableRootsWithoutWriteAccess,
        PolicyError::WritableRootCoversHiddenRoot {
            root: "/home/me".to_string(),
            hidden: "/home/me/.config/pm3".to_string(),
        },
        PolicyError::ForbiddenWritableRoot("/etc".to_string()),
    ];
    for err in errors {
        assert!(
            err.to_string().starts_with("cannot accept"),
            "error message must start with a verb: {err}"
        );
    }
}

#[test]
fn validate_rejects_a_working_directory_that_would_reopen_a_hidden_root() {
    let policy = SandboxPolicy {
        writable_roots: Vec::new(),
        derived_roots: vec!["/home/me/.pm3".to_string()],
        unreadable_roots: vec!["/home/me/.pm3".to_string()],
        ..workspace_policy()
    };
    let err = validate_policy(&policy).unwrap_err();
    assert!(
        matches!(err, PolicyError::WritableRootCoversHiddenRoot { .. }),
        "got: {err}"
    );
}

#[test]
fn validate_accepts_a_working_directory_below_a_hidden_root() {
    let policy = SandboxPolicy {
        writable_roots: Vec::new(),
        derived_roots: vec!["/home/me/.pm3/api".to_string()],
        unreadable_roots: vec!["/home/me/.pm3".to_string()],
        ..workspace_policy()
    };
    validate_policy(&policy).expect("pm3 masks the home and binds the workspace back in");
}
