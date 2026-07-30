use super::*;

#[test]
fn delivery_error_names_the_pid_and_reason() {
    let err = SignalError::Delivery {
        pid: 4242,
        reason: "no such process".to_string(),
    };
    assert_eq!(err.to_string(), "cannot signal pid 4242: no such process");
}
