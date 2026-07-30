use super::*;

#[test]
fn read_error_names_the_path_and_reason() {
    let err = FingerprintError::Read {
        path: "/usr/bin/node".to_string(),
        reason: "no such file or directory".to_string(),
    };
    assert_eq!(
        err.to_string(),
        "cannot digest '/usr/bin/node': no such file or directory"
    );
}
