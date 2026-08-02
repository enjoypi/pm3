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

#[tokio::test]
async fn a_launcher_tracks_the_pids_it_handed_out() {
    let ports = crate::ports_test_helpers::FakePorts::new(1000);
    ports.adopt(4321).await;
    assert_eq!(ports.tracked_pids().await, vec![4321]);
}

#[tokio::test]
async fn a_launcher_that_handed_out_nothing_tracks_nothing() {
    let ports = crate::ports_test_helpers::FakePorts::new(1000);
    assert!(ports.tracked_pids().await.is_empty());
}
