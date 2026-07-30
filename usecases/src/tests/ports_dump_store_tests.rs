use super::*;

#[test]
fn read_error_names_the_path_and_reason() {
    let err = DumpError::Read {
        path: "/home/u/.pm3/dump.yaml".to_string(),
        reason: "invalid yaml".to_string(),
    };
    assert_eq!(
        err.to_string(),
        "cannot read state file '/home/u/.pm3/dump.yaml': invalid yaml"
    );
}

#[test]
fn write_error_names_the_path_and_reason() {
    let err = DumpError::Write {
        path: "/home/u/.pm3/dump.yaml".to_string(),
        reason: "disk full".to_string(),
    };
    assert_eq!(
        err.to_string(),
        "cannot write state file '/home/u/.pm3/dump.yaml': disk full"
    );
}
