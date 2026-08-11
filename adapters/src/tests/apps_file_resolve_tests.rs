use super::*;

#[test]
fn resolve_specs_grants_no_writable_root_in_full_access_mode() {
    let defaults = SpecDefaults {
        sandbox_mode: SandboxMode::DangerFullAccess,
        ..defaults()
    };
    let spec = resolve_one(&defaults, &minimal_entry());
    assert!(
        spec.sandbox.granted_roots().is_empty(),
        "got: {:?}",
        spec.sandbox.granted_roots()
    );
}

#[test]
fn resolve_specs_honours_explicit_writable_roots() {
    let entry = AppEntry {
        sandbox: Some(SandboxEntry {
            writable_roots: Some(vec!["/srv/web/var".to_string()]),
            ..sandbox_entry()
        }),
        ..minimal_entry()
    };
    let spec = resolve_one(&defaults(), &entry);
    assert_eq!(spec.sandbox.writable_roots, vec!["/srv/web/var"]);
}

#[test]
fn resolve_specs_keeps_the_workspace_defaults_beside_an_empty_writable_roots_list() {
    let entry = AppEntry {
        sandbox: Some(SandboxEntry {
            writable_roots: Some(Vec::new()),
            ..sandbox_entry()
        }),
        ..minimal_entry()
    };
    let spec = resolve_one(&defaults(), &entry);
    assert!(
        spec.sandbox.granted_roots().contains(&spec.cwd.as_str()),
        "a service always owns its own working directory: {:?}",
        spec.sandbox.granted_roots()
    );
}

#[test]
fn resolve_specs_keeps_the_workspace_defaults_beside_a_declared_writable_root() {
    let entry = AppEntry {
        sandbox: Some(SandboxEntry {
            writable_roots: Some(vec!["/srv/data".to_string()]),
            ..sandbox_entry()
        }),
        ..minimal_entry()
    };
    let spec = resolve_one(&defaults(), &entry);
    let granted = spec.sandbox.granted_roots();
    assert!(
        granted.contains(&spec.cwd.as_str()) && granted.contains(&"/srv/data"),
        "declaring an extra root must not drop the defaults: {granted:?}"
    );
}

#[test]
fn a_declared_writable_root_stays_out_of_the_derived_set() {
    let entry = AppEntry {
        sandbox: Some(SandboxEntry {
            writable_roots: Some(vec!["/srv/data".to_string()]),
            ..sandbox_entry()
        }),
        ..minimal_entry()
    };
    let spec = resolve_one(&defaults(), &entry);
    assert_eq!(spec.sandbox.writable_roots, vec!["/srv/data".to_string()]);
}

#[test]
fn spec_defaults_reads_the_pm3_sandbox_section() {
    let mut pm3 = pm3_config(SandboxMode::WorkspaceWrite.as_str());
    pm3.sandbox.network = true;
    let defaults = SpecDefaults::from_config(&pm3, HOME_DIR, CFG_DIR, LOGS_DIR, Some(TMP_DIR))
        .expect("should build");
    assert_eq!(defaults.sandbox_mode, SandboxMode::WorkspaceWrite);
    assert!(defaults.sandbox_network);
    assert_eq!(defaults.restart.max_restarts, pm3.restart.max_restarts);
}

#[test]
fn spec_defaults_rejects_an_unknown_configured_sandbox_mode() {
    let mut pm3 = pm3_config(SandboxMode::WorkspaceWrite.as_str());
    pm3.sandbox.mode = "yolo".to_string();
    let err = SpecDefaults::from_config(&pm3, HOME_DIR, CFG_DIR, LOGS_DIR, Some(TMP_DIR))
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("sandbox mode 'yolo' for pm3.sandbox"),
        "got: {err}"
    );
}

#[test]
fn a_script_that_is_not_on_the_search_path_is_rejected() {
    let mut entry = minimal_entry();
    entry.script = "pm3-not-a-real-program".to_string();
    let err = resolve_checked(&defaults(), &entry)
        .unwrap_err()
        .to_string();
    assert_eq!(
        err,
        "cannot find 'pm3-not-a-real-program' for app 'web' on pm3.search_path"
    );
}

#[test]
fn a_schedule_reaches_the_spec() {
    let entry = AppEntry {
        schedule: Some("~ * * * *".to_string()),
        ..minimal_entry()
    };
    let spec = resolve_checked(&defaults(), &entry).expect("a valid schedule should resolve");
    assert_eq!(spec.schedule.as_deref(), Some("~ * * * *"));
}

#[test]
fn an_app_without_a_schedule_resolves_to_none() {
    let spec = resolve_checked(&defaults(), &minimal_entry()).expect("resolve");
    assert_eq!(spec.schedule, None);
}

#[test]
fn an_unparsable_schedule_is_rejected_at_load_time() {
    let entry = AppEntry {
        schedule: Some("nonsense".to_string()),
        ..minimal_entry()
    };
    let err = resolve_checked(&defaults(), &entry).unwrap_err();
    assert!(matches!(err, AppsFileError::Cron(_)), "got: {err}");
}

#[test]
fn an_unexpandable_schedule_is_rejected_at_load_time() {
    let entry = AppEntry {
        schedule: Some("0~59/0 * * * *".to_string()),
        ..minimal_entry()
    };
    let err = resolve_checked(&defaults(), &entry).unwrap_err();
    assert!(err.to_string().contains("step 0"), "got: {err}");
}

#[test]
fn resolve_specs_refuses_a_forbidden_writable_root() {
    let entry = AppEntry {
        sandbox: Some(SandboxEntry {
            writable_roots: Some(vec!["/etc".to_string()]),
            ..sandbox_entry()
        }),
        ..minimal_entry()
    };
    let err = resolve_one_err(&defaults(), &entry);
    assert!(err.contains("forbidden_writable_roots"), "got: {err}");
}

#[test]
fn resolve_specs_reads_a_declared_memory_limit() {
    let entry = AppEntry {
        max_memory: Some("300M".to_string()),
        ..minimal_entry()
    };
    let spec = resolve_one(&defaults(), &entry);
    assert_eq!(spec.max_memory_kib, Some(300 * 1024));
}

#[test]
fn resolve_specs_leaves_an_undeclared_memory_limit_open() {
    let spec = resolve_one(&defaults(), &minimal_entry());
    assert_eq!(spec.max_memory_kib, None);
}

#[test]
fn resolve_specs_refuses_an_unreadable_memory_limit() {
    let entry = AppEntry {
        max_memory: Some("plenty".to_string()),
        ..minimal_entry()
    };
    let err = resolve_one_err(&defaults(), &entry);
    assert!(err.contains("max_memory 'plenty'"), "got: {err}");
}

#[test]
fn resolve_specs_refuses_an_unknown_read_scope() {
    let entry = AppEntry {
        sandbox: Some(SandboxEntry {
            read: Some("everything".to_string()),
            ..sandbox_entry()
        }),
        ..minimal_entry()
    };
    let err = resolve_one_err(&defaults(), &entry);
    assert!(err.contains("sandbox read 'everything'"), "got: {err}");
}

#[test]
fn resolve_specs_honours_a_declared_read_scope() {
    let entry = AppEntry {
        sandbox: Some(SandboxEntry {
            read: Some(ReadScope::Full.as_str().to_string()),
            ..sandbox_entry()
        }),
        ..minimal_entry()
    };
    let spec = resolve_one(&defaults(), &entry);
    assert_eq!(spec.sandbox.read, ReadScope::Full);
}

#[test]
fn resolve_specs_keep_the_pm3_directories_out_of_every_sandbox() {
    let spec = resolve_one(&defaults(), &minimal_entry());
    assert_eq!(spec.sandbox.hidden_paths(), vec![HOME_DIR, CFG_DIR]);
}

#[test]
fn spec_defaults_rejects_an_unknown_configured_read_scope() {
    let mut pm3 = pm3_config(SandboxMode::WorkspaceWrite.as_str());
    pm3.sandbox.read = "everything".to_string();
    let err = SpecDefaults::from_config(&pm3, HOME_DIR, CFG_DIR, LOGS_DIR, Some(TMP_DIR))
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("sandbox read 'everything' for pm3.sandbox"),
        "got: {err}"
    );
}

#[test]
fn resolve_specs_reads_an_exec_ready_probe() {
    let entry = AppEntry {
        ready_probe: Some(ReadyProbeEntry {
            exec: Some(vec!["curl".to_string(), "-sf".to_string()]),
            tcp: None,
        }),
        listen_timeout_ms: Some(20000),
        ..minimal_entry()
    };
    let spec = resolve_one(&defaults(), &entry);
    assert_eq!(
        spec.ready_probe,
        Some(ReadyProbe::Exec {
            command: vec!["curl".to_string(), "-sf".to_string()]
        })
    );
    assert_eq!(spec.listen_timeout_ms, Some(20000));
}

#[test]
fn resolve_specs_reads_a_tcp_ready_probe() {
    let entry = AppEntry {
        ready_probe: Some(ReadyProbeEntry {
            exec: None,
            tcp: Some("127.0.0.1:8080".to_string()),
        }),
        ..minimal_entry()
    };
    let spec = resolve_one(&defaults(), &entry);
    assert_eq!(
        spec.ready_probe,
        Some(ReadyProbe::Tcp {
            host: "127.0.0.1".to_string(),
            port: 8080,
        })
    );
}

#[test]
fn resolve_specs_leaves_an_undeclared_probe_absent() {
    let spec = resolve_one(&defaults(), &minimal_entry());
    assert_eq!(spec.ready_probe, None);
    assert_eq!(spec.listen_timeout_ms, None);
}

#[test]
fn resolve_specs_refuses_a_probe_with_both_kinds() {
    let entry = AppEntry {
        ready_probe: Some(ReadyProbeEntry {
            exec: Some(vec!["true".to_string()]),
            tcp: Some("127.0.0.1:8080".to_string()),
        }),
        ..minimal_entry()
    };
    let err = resolve_one_err(&defaults(), &entry);
    assert!(err.contains("exactly one"), "got: {err}");
}

#[test]
fn resolve_specs_refuses_a_probe_with_neither_kind() {
    let entry = AppEntry {
        ready_probe: Some(ReadyProbeEntry {
            exec: None,
            tcp: None,
        }),
        ..minimal_entry()
    };
    let err = resolve_one_err(&defaults(), &entry);
    assert!(err.contains("declare exec or tcp"), "got: {err}");
}

#[test]
fn resolve_specs_refuses_a_tcp_probe_without_a_port() {
    let entry = AppEntry {
        ready_probe: Some(ReadyProbeEntry {
            exec: None,
            tcp: Some("127.0.0.1".to_string()),
        }),
        ..minimal_entry()
    };
    let err = resolve_one_err(&defaults(), &entry);
    assert!(err.contains("host:port"), "got: {err}");
}

#[test]
fn resolve_specs_refuses_a_tcp_probe_with_a_non_numeric_port() {
    let entry = AppEntry {
        ready_probe: Some(ReadyProbeEntry {
            exec: None,
            tcp: Some("127.0.0.1:http".to_string()),
        }),
        ..minimal_entry()
    };
    let err = resolve_one_err(&defaults(), &entry);
    assert!(err.contains("1-65535"), "got: {err}");
}

#[test]
fn resolve_specs_carries_stop_exit_codes() {
    let entry = AppEntry {
        stop_exit_codes: vec![0, 3],
        ..minimal_entry()
    };
    let spec = resolve_one(&defaults(), &entry);
    assert_eq!(spec.stop_exit_codes, vec![0, 3]);
}

#[test]
fn a_service_file_reads_stop_exit_codes() {
    let entry = parse_service_file("name: web\nscript: /bin/sh\nstop_exit_codes:\n  - 3\n  - 0\n")
        .expect("should parse");
    assert_eq!(entry.stop_exit_codes, vec![3, 0]);
}
