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
