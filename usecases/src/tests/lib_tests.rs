use super::*;

fn assert_transparent(wrapped: &UsecaseError, expected: &str) {
    assert_eq!(wrapped.to_string(), expected);
}

#[test]
fn spec_errors_render_transparently() {
    let source = SpecError::EmptyName;
    let expected = source.to_string();
    assert_transparent(&UsecaseError::from(source), &expected);
}

#[test]
fn dependency_errors_render_transparently() {
    let source = DependencyError::Cycle {
        involved: vec!["a".to_string()],
    };
    let expected = source.to_string();
    assert_transparent(&UsecaseError::from(source), &expected);
}

#[test]
fn policy_errors_render_transparently() {
    let source = PolicyError::EmptyWritableRoot;
    let expected = source.to_string();
    assert_transparent(&UsecaseError::from(source), &expected);
}

#[test]
fn launch_errors_render_transparently() {
    let source = LaunchError::Spawn {
        app: "api".to_string(),
        reason: "boom".to_string(),
    };
    let expected = source.to_string();
    assert_transparent(&UsecaseError::from(source), &expected);
}

#[test]
fn signal_errors_render_transparently() {
    let source = SignalError::Delivery {
        pid: 1,
        reason: "boom".to_string(),
    };
    let expected = source.to_string();
    assert_transparent(&UsecaseError::from(source), &expected);
}

#[test]
fn sandbox_errors_render_transparently() {
    let source = SandboxError::NoBackend {
        app: "api".to_string(),
    };
    let expected = source.to_string();
    assert_transparent(&UsecaseError::from(source), &expected);
}

#[test]
fn dump_errors_render_transparently() {
    let source = DumpError::Read {
        path: "/dump.yaml".to_string(),
        reason: "boom".to_string(),
    };
    let expected = source.to_string();
    assert_transparent(&UsecaseError::from(source), &expected);
}

#[test]
fn fingerprint_errors_render_transparently() {
    let source = FingerprintError::Read {
        path: "/usr/bin/node".to_string(),
        reason: "boom".to_string(),
    };
    let expected = source.to_string();
    assert_transparent(&UsecaseError::from(source), &expected);
}

#[test]
fn not_found_names_the_selector() {
    let err = UsecaseError::NotFound("api".to_string());
    assert_eq!(err.to_string(), "cannot find app 'api'");
}
