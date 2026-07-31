use super::{test_helpers::*, *};

#[test]
fn validate_accepts_a_minimal_spec() {
    validate_spec(&spec("api")).expect("fixture should validate");
}

#[test]
fn validate_rejects_blank_name() {
    let candidate = AppSpec {
        name: "   ".to_string(),
        ..spec("api")
    };
    let err = validate_spec(&candidate).unwrap_err();
    assert_eq!(err, SpecError::EmptyName);
}

#[test]
fn validate_rejects_an_all_digit_name() {
    let candidate = AppSpec {
        name: "3".to_string(),
        ..spec("api")
    };
    let err = validate_spec(&candidate).unwrap_err();
    assert_eq!(err, SpecError::NumericName("3".to_string()));
}

#[test]
fn validate_accepts_a_name_that_merely_contains_digits() {
    let candidate = AppSpec {
        name: "api2".to_string(),
        ..spec("api")
    };
    validate_spec(&candidate).expect("a name mixing letters and digits should validate");
}

#[test]
fn validate_rejects_blank_script() {
    let candidate = AppSpec {
        script: "  ".to_string(),
        ..spec("api")
    };
    let err = validate_spec(&candidate).unwrap_err();
    assert_eq!(err, SpecError::EmptyScript("api".to_string()));
}

#[test]
fn validate_rejects_relative_cwd() {
    let candidate = AppSpec {
        cwd: "srv/app".to_string(),
        ..spec("api")
    };
    let err = validate_spec(&candidate).unwrap_err();
    assert_eq!(
        err,
        SpecError::RelativeCwd {
            app: "api".to_string(),
            cwd: "srv/app".to_string(),
        }
    );
}

#[test]
fn validate_rejects_self_dependency() {
    let candidate = AppSpec {
        depends_on: vec!["api".to_string()],
        ..spec("api")
    };
    let err = validate_spec(&candidate).unwrap_err();
    assert_eq!(err, SpecError::SelfDependency("api".to_string()));
}

#[test]
fn validate_rejects_zero_min_uptime() {
    let candidate = AppSpec {
        min_uptime_ms: 0,
        ..spec("api")
    };
    let err = validate_spec(&candidate).unwrap_err();
    assert_eq!(err, SpecError::InvalidMinUptime("api".to_string()));
}

#[test]
fn validate_rejects_empty_env_key() {
    let candidate = AppSpec {
        env: vec![(String::new(), "value".to_string())],
        ..spec("api")
    };
    let err = validate_spec(&candidate).unwrap_err();
    assert_eq!(err, SpecError::EmptyEnvKey("api".to_string()));
}

#[test]
fn validate_rejects_a_blank_schedule() {
    let candidate = AppSpec {
        schedule: Some("   ".to_string()),
        ..spec("api")
    };
    let err = validate_spec(&candidate).unwrap_err();
    assert_eq!(err, SpecError::EmptySchedule("api".to_string()));
}

#[test]
fn validate_accepts_a_schedule_without_parsing_it() {
    let candidate = AppSpec {
        schedule: Some("not a cron expression".to_string()),
        ..spec("api")
    };
    validate_spec(&candidate).expect("syntax belongs to adapters, not entities");
}

#[test]
fn validate_propagates_sandbox_policy_errors() {
    let candidate = AppSpec {
        sandbox: SandboxPolicy {
            writable_roots: vec!["relative".to_string()],
            ..confined_policy()
        },
        ..spec("api")
    };
    let err = validate_spec(&candidate).unwrap_err();
    assert_eq!(
        err,
        SpecError::Sandbox {
            app: "api".to_string(),
            source: PolicyError::RelativeWritableRoot("relative".to_string()),
        }
    );
}

#[test]
fn restart_policy_mirrors_the_spec_fields() {
    let candidate = AppSpec {
        autorestart: false,
        min_uptime_ms: 500,
        max_restarts: 3,
        restart_delay_ms: 100,
        ..spec("api")
    };
    assert_eq!(
        candidate.restart_policy(),
        RestartPolicy {
            autorestart: false,
            min_uptime_ms: 500,
            max_restarts: 3,
            restart_delay_ms: 100,
        }
    );
}

#[test]
fn dependency_node_borrows_name_and_dependencies() {
    let candidate = AppSpec {
        depends_on: vec!["db".to_string()],
        ..spec("api")
    };
    let node = candidate.dependency_node();
    assert_eq!(node.name, "api");
    assert_eq!(node.depends_on, ["db".to_string()]);
}

#[test]
fn every_spec_error_renders_a_message() {
    let errors = [
        SpecError::EmptyName,
        SpecError::NumericName("3".to_string()),
        SpecError::EmptyScript("api".to_string()),
        SpecError::RelativeCwd {
            app: "api".to_string(),
            cwd: "srv".to_string(),
        },
        SpecError::SelfDependency("api".to_string()),
        SpecError::InvalidMinUptime("api".to_string()),
        SpecError::EmptyEnvKey("api".to_string()),
        SpecError::EmptySchedule("api".to_string()),
        SpecError::Sandbox {
            app: "api".to_string(),
            source: PolicyError::EmptyWritableRoot,
        },
    ];
    for err in errors {
        assert!(
            err.to_string().starts_with("cannot accept"),
            "error message must start with a verb: {err}"
        );
    }
}

#[test]
fn a_schedule_without_autorestart_is_a_one_shot_task() {
    let candidate = AppSpec {
        autorestart: false,
        schedule: Some("* * * * *".to_string()),
        ..spec("sweep")
    };
    assert!(candidate.is_scheduled_task());
}

#[test]
fn a_schedule_with_autorestart_stays_a_long_running_service() {
    let candidate = AppSpec {
        autorestart: true,
        schedule: Some("* * * * *".to_string()),
        ..spec("api")
    };
    assert!(!candidate.is_scheduled_task());
}

#[test]
fn an_app_without_a_schedule_is_never_a_task() {
    let candidate = AppSpec {
        autorestart: false,
        ..spec("api")
    };
    assert!(!candidate.is_scheduled_task());
}
