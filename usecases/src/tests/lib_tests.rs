use entities::DependencyError;

use super::*;

fn transparent<E: std::fmt::Display>(source: E) -> (UsecaseError, String)
where
    UsecaseError: From<E>,
{
    let expected = source.to_string();
    (UsecaseError::from(source), expected)
}

#[test]
fn wrapped_errors_render_transparently() {
    let cases = [
        transparent(SpecError::EmptyName),
        transparent(DependencyError::Cycle {
            involved: vec!["a".to_string()],
        }),
        transparent(PolicyError::EmptyWritableRoot),
        transparent(LaunchError::Spawn {
            app: "api".to_string(),
            reason: "boom".to_string(),
        }),
        transparent(SignalError::Delivery {
            pid: 1,
            reason: "boom".to_string(),
        }),
        transparent(SandboxError::NoBackend {
            app: "api".to_string(),
        }),
        transparent(DumpError::Read {
            path: "/dump.yaml".to_string(),
            reason: "boom".to_string(),
        }),
        transparent(FingerprintError::Read {
            path: "/usr/bin/node".to_string(),
            reason: "boom".to_string(),
        }),
        transparent(entities::SignalNameError {
            raw: "KILL9".to_string(),
        }),
    ];
    for (wrapped, expected) in cases {
        assert_eq!(wrapped.to_string(), expected);
    }
}

#[test]
fn not_found_names_the_selector() {
    let err = UsecaseError::NotFound("api".to_string());
    assert_eq!(err.to_string(), "cannot find app 'api'");
}

#[test]
fn not_running_names_the_app() {
    let err = UsecaseError::NotRunning("api".to_string());
    assert_eq!(err.to_string(), "cannot signal 'api': it is not running");
}
