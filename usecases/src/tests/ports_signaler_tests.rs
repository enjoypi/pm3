use super::*;

#[test]
fn delivery_error_names_the_pid_and_reason() {
    let err = SignalError::Delivery {
        pid: 4242,
        reason: "no such process".to_string(),
    };
    assert_eq!(err.to_string(), "cannot signal pid 4242: no such process");
}

#[test]
fn a_group_scope_reaches_the_whole_process_group() {
    assert!(SignalScope::ProcessGroup.reaches_the_group());
}

#[test]
fn a_single_pid_scope_stays_off_the_process_group() {
    assert!(!SignalScope::SinglePid.reaches_the_group());
}
