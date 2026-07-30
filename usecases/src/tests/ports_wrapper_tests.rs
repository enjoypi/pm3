use super::*;

#[test]
fn missing_backend_error_tells_the_operator_how_to_recover() {
    let err = SandboxError::NoBackend {
        app: "api".to_string(),
    };
    let message = err.to_string();
    assert!(
        message.starts_with("cannot confine app 'api'"),
        "got: {message}"
    );
    assert!(message.contains("bubblewrap"), "got: {message}");
    assert!(message.contains("danger-full-access"), "got: {message}");
}

#[test]
fn unsupported_policy_error_names_the_reason() {
    let err = SandboxError::Unsupported {
        app: "api".to_string(),
        reason: "writable root escapes the sandbox".to_string(),
    };
    assert_eq!(
        err.to_string(),
        "cannot confine app 'api': writable root escapes the sandbox"
    );
}
