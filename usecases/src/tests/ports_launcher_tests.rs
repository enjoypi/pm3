use super::*;

#[test]
fn exit_code_zero_did_not_fail() {
    assert!(!ExitOutcome::Code(0).failed());
}

#[test]
fn a_nonzero_exit_code_failed() {
    assert!(ExitOutcome::Code(1).failed());
}

#[test]
fn a_signalled_child_failed() {
    assert!(ExitOutcome::Signalled.failed());
}

#[test]
fn an_exit_pm3_could_not_observe_is_not_a_failure() {
    assert!(!ExitOutcome::Unobserved.failed());
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

#[test]
fn every_outcome_names_itself_for_the_log() {
    assert_eq!(ExitOutcome::Code(0).as_str(), "code");
    assert_eq!(ExitOutcome::Signalled.as_str(), "signalled");
    assert_eq!(ExitOutcome::Unobserved.as_str(), "unobserved");
}

#[test]
fn only_a_code_carries_a_number() {
    assert_eq!(ExitOutcome::Code(3).code(), Some(3));
    assert_eq!(ExitOutcome::Signalled.code(), None);
    assert_eq!(ExitOutcome::Unobserved.code(), None);
}
