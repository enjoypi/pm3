use super::*;

#[test]
fn a_scan_error_names_the_directory_and_reason() {
    let err = LogRotateError::Scan {
        path: "/home/u/.pm3/logs".to_string(),
        reason: "permission denied".to_string(),
    };
    assert_eq!(
        err.to_string(),
        "cannot scan the log directory '/home/u/.pm3/logs': permission denied"
    );
}
