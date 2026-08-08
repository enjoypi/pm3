use super::*;

#[test]
fn an_exec_probe_needs_a_program() {
    let err = validate_probe(
        "api",
        &ReadyProbe::Exec {
            command: Vec::new(),
        },
    )
    .unwrap_err();
    assert_eq!(err, SpecError::EmptyReadyProbe("api".to_string()));
}

#[test]
fn an_exec_probe_rejects_a_blank_program() {
    let probe = ReadyProbe::Exec {
        command: vec!["  ".to_string()],
    };
    let err = validate_probe("api", &probe).unwrap_err();
    assert_eq!(err, SpecError::EmptyReadyProbe("api".to_string()));
}

#[test]
fn an_exec_probe_with_a_program_is_accepted() {
    let probe = ReadyProbe::Exec {
        command: vec!["curl".to_string(), "-sf".to_string()],
    };
    validate_probe("api", &probe).expect("a real command should validate");
}

#[test]
fn a_tcp_probe_rejects_a_blank_host() {
    let probe = ReadyProbe::Tcp {
        host: " ".to_string(),
        port: 8080,
    };
    let err = validate_probe("api", &probe).unwrap_err();
    assert_eq!(
        err,
        SpecError::InvalidReadyEndpoint {
            app: "api".to_string(),
            endpoint: " :8080".to_string(),
        }
    );
}

#[test]
fn a_tcp_probe_rejects_port_zero() {
    let probe = ReadyProbe::Tcp {
        host: "127.0.0.1".to_string(),
        port: 0,
    };
    let err = validate_probe("api", &probe).unwrap_err();
    assert!(
        matches!(err, SpecError::InvalidReadyEndpoint { .. }),
        "got: {err}"
    );
}

#[test]
fn a_tcp_probe_with_a_real_endpoint_is_accepted() {
    let probe = ReadyProbe::Tcp {
        host: "127.0.0.1".to_string(),
        port: 8080,
    };
    validate_probe("api", &probe).expect("a real endpoint should validate");
}
