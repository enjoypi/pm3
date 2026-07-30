use super::*;

#[test]
fn exit_code_zero_is_clean() {
    assert!(ExitOutcome { exit_code: Some(0) }.clean());
}

#[test]
fn nonzero_exit_code_is_not_clean() {
    assert!(!ExitOutcome { exit_code: Some(1) }.clean());
}

#[test]
fn missing_exit_code_is_not_clean() {
    assert!(!ExitOutcome { exit_code: None }.clean());
}

#[test]
fn spawn_error_names_the_app_and_reason() {
    let err = LaunchError::Spawn {
        app: "api".to_string(),
        reason: "no such file".to_string(),
    };
    assert_eq!(err.to_string(), "cannot spawn app 'api': no such file");
}

#[test]
fn log_file_error_names_the_path() {
    let err = LaunchError::LogFile {
        app: "api".to_string(),
        path: "/logs/api-out.log".to_string(),
        reason: "permission denied".to_string(),
    };
    assert_eq!(
        err.to_string(),
        "cannot open log file '/logs/api-out.log' for app 'api': permission denied"
    );
}
