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
fn validate_accepts_the_punctuation_a_file_name_can_carry() {
    validate_app_name("api-2_0.worker").expect("dashes, underscores and dots are safe");
}

#[test]
fn validate_rejects_a_name_that_would_escape_the_service_directory() {
    let err = validate_app_name("../evil").unwrap_err();
    assert_eq!(err, SpecError::DottedName("../evil".to_string()));
}

#[test]
fn validate_rejects_the_reserved_every_app_selector() {
    let err = validate_app_name("all").unwrap_err();
    assert_eq!(err, SpecError::ReservedName("all".to_string()));
}

#[test]
fn validate_rejects_a_name_that_would_shadow_an_encrypted_environment() {
    let err = validate_app_name("api.enc").unwrap_err();
    assert_eq!(err, SpecError::EncryptedName("api.enc".to_string()));
}

#[test]
fn validate_rejects_the_daemon_config_file_name() {
    let err = validate_app_name("config").unwrap_err();
    assert_eq!(err, SpecError::ReservedFileName("config".to_string()));
}

#[test]
fn validate_rejects_the_global_environment_file_name() {
    let err = validate_app_name("pm3").unwrap_err();
    assert_eq!(err, SpecError::ReservedFileName("pm3".to_string()));
}

#[test]
fn validate_accepts_a_name_that_merely_starts_with_a_reserved_word() {
    validate_app_name("configurator").expect("only the exact name is reserved");
    validate_app_name("pm3-agent").expect("only the exact name is reserved");
}

#[test]
fn validate_accepts_a_name_that_merely_holds_enc() {
    validate_app_name("enc.api").expect("only the tail is reserved");
}

#[test]
fn validate_rejects_a_hidden_name() {
    let err = validate_app_name(".hidden").unwrap_err();
    assert_eq!(err, SpecError::DottedName(".hidden".to_string()));
}

#[test]
fn validate_rejects_a_path_separator_inside_a_name() {
    let err = validate_app_name("team/api").unwrap_err();
    assert_eq!(
        err,
        SpecError::UnsafeName {
            name: "team/api".to_string(),
            character: '/',
        }
    );
}

#[test]
fn validate_rejects_a_space_inside_a_name() {
    let err = validate_app_name("my app").unwrap_err();
    assert_eq!(
        err,
        SpecError::UnsafeName {
            name: "my app".to_string(),
            character: ' ',
        }
    );
}

#[test]
fn a_spec_carrying_an_unsafe_name_is_refused() {
    let candidate = AppSpec {
        name: "team/api".to_string(),
        ..spec("api")
    };
    let err = validate_spec(&candidate).unwrap_err();
    assert!(matches!(err, SpecError::UnsafeName { .. }), "got: {err}");
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
fn validate_rejects_zero_max_restart_delay() {
    let candidate = AppSpec {
        max_restart_delay_ms: 0,
        ..spec("api")
    };
    let err = validate_spec(&candidate).unwrap_err();
    assert_eq!(err, SpecError::InvalidMaxRestartDelay("api".to_string()));
}

#[test]
fn validate_rejects_a_probe_without_a_command() {
    let candidate = AppSpec {
        ready_probe: Some(ReadyProbe::Exec {
            command: Vec::new(),
        }),
        ..spec("api")
    };
    let err = validate_spec(&candidate).unwrap_err();
    assert_eq!(err, SpecError::EmptyReadyProbe("api".to_string()));
}

#[test]
fn validate_accepts_a_real_probe() {
    let candidate = AppSpec {
        ready_probe: Some(ReadyProbe::Tcp {
            host: "127.0.0.1".to_string(),
            port: 8080,
        }),
        listen_timeout_ms: Some(5000),
        ..spec("api")
    };
    validate_spec(&candidate).expect("a real probe should validate");
}

#[test]
fn validate_rejects_zero_listen_timeout() {
    let candidate = AppSpec {
        listen_timeout_ms: Some(0),
        ..spec("api")
    };
    let err = validate_spec(&candidate).unwrap_err();
    assert_eq!(err, SpecError::InvalidListenTimeout("api".to_string()));
}

#[test]
fn validate_rejects_empty_env_key() {
    let candidate = AppSpec {
        env: vec![EnvValue::app("", "value")],
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
        max_restart_delay_ms: 15000,
        ..spec("api")
    };
    assert_eq!(
        candidate.restart_policy(),
        RestartPolicy {
            autorestart: false,
            min_uptime_ms: 500,
            max_restarts: 3,
            restart_delay_ms: 100,
            max_restart_delay_ms: 15000,
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
        SpecError::DottedName(".api".to_string()),
        SpecError::ReservedName("all".to_string()),
        SpecError::EncryptedName("api.enc".to_string()),
        SpecError::ReservedFileName("config".to_string()),
        SpecError::UnsafeName {
            name: "my app".to_string(),
            character: ' ',
        },
        SpecError::EmptyScript("api".to_string()),
        SpecError::RelativeCwd {
            app: "api".to_string(),
            cwd: "srv".to_string(),
        },
        SpecError::SelfDependency("api".to_string()),
        SpecError::InvalidMinUptime("api".to_string()),
        SpecError::InvalidMaxRestartDelay("api".to_string()),
        SpecError::EmptyReadyProbe("api".to_string()),
        SpecError::InvalidReadyEndpoint {
            app: "api".to_string(),
            endpoint: ":0".to_string(),
        },
        SpecError::InvalidListenTimeout("api".to_string()),
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

#[test]
fn a_listed_exit_code_is_a_clean_stop() {
    let candidate = AppSpec {
        stop_exit_codes: vec![3],
        ..spec("api")
    };
    assert!(candidate.stops_on(3));
    assert!(!candidate.stops_on(7));
}

#[test]
fn validate_rejects_a_stop_exit_code_above_255() {
    let candidate = AppSpec {
        stop_exit_codes: vec![256],
        ..spec("api")
    };
    let err = validate_spec(&candidate).unwrap_err();
    assert_eq!(
        err.to_string(),
        "cannot accept stop_exit_code 256 for app 'api': use a code of 0-255"
    );
}

#[test]
fn validate_rejects_a_negative_stop_exit_code() {
    let candidate = AppSpec {
        stop_exit_codes: vec![-1],
        ..spec("api")
    };
    let err = validate_spec(&candidate).unwrap_err();
    assert_eq!(
        err.to_string(),
        "cannot accept stop_exit_code -1 for app 'api': use a code of 0-255"
    );
}

#[test]
fn a_liveness_probe_is_validated_like_a_ready_probe() {
    let candidate = AppSpec {
        liveness_probe: Some(ReadyProbe::Tcp {
            host: String::new(),
            port: 8080,
        }),
        ..spec("api")
    };
    assert_eq!(
        validate_spec(&candidate),
        Err(SpecError::InvalidReadyEndpoint {
            app: "api".to_string(),
            endpoint: ":8080".to_string(),
        })
    );
}

#[test]
fn a_tcp_liveness_probe_is_accepted() {
    let candidate = AppSpec {
        liveness_probe: Some(ReadyProbe::Tcp {
            host: "127.0.0.1".to_string(),
            port: 8080,
        }),
        ..spec("api")
    };
    assert_eq!(validate_spec(&candidate), Ok(()));
}

#[test]
fn an_exec_liveness_probe_is_refused() {
    let candidate = AppSpec {
        liveness_probe: Some(ReadyProbe::Exec {
            command: vec!["/usr/bin/true".to_string()],
        }),
        ..spec("api")
    };
    assert_eq!(
        validate_spec(&candidate),
        Err(SpecError::ExecLivenessProbe("api".to_string())),
        "a liveness probe runs forever, so it must not fork"
    );
}

#[test]
fn every_environment_origin_has_a_name() {
    assert_eq!(crate::EnvOrigin::Plain.as_str(), "plain");
    assert_eq!(crate::EnvOrigin::Encrypted.as_str(), "encrypted");
    assert_eq!(crate::EnvOrigin::Sealed.as_str(), "sealed");
    assert_eq!(crate::EnvOrigin::default(), crate::EnvOrigin::Plain);
}

#[test]
fn the_declared_env_count_excludes_the_values_pm3_injects() {
    let spec = AppSpec {
        env: vec![
            EnvValue::injected("HOME", "/home/dev"),
            EnvValue::global("TZ", "UTC"),
            EnvValue::app("PORT", "8080"),
        ],
        ..spec("api")
    };
    let counted = spec.declared_env_count();
    assert_eq!(counted, 2, "got: {counted}");
}
