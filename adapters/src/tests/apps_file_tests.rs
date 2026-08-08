use usecases::{ReadScope, RestartPolicy, SandboxMode};

use super::{test_helpers::*, *};

#[test]
fn parse_apps_file_reads_a_minimal_entry() {
    let apps = parse_apps_file(&minimal_yaml()).expect("should parse");
    let [entry] = apps.apps.as_slice() else {
        panic!("expected exactly one entry, got {:?}", apps.apps);
    };
    assert_eq!(entry.name, APP_NAME);
    assert_eq!(entry.script, SCRIPT);
    assert_eq!(entry.cwd.as_deref(), Some(CWD));
}

#[test]
fn parse_apps_file_defaults_every_optional_field_of_a_minimal_entry() {
    let apps = parse_apps_file(&minimal_yaml()).expect("should parse");
    let entry = apps.apps.first().expect("one entry");
    assert!(entry.args.is_empty(), "got: {:?}", entry.args);
    assert!(entry.rejected_env.is_none());
    assert!(entry.depends_on.is_empty(), "got: {:?}", entry.depends_on);
    assert_eq!(entry.autorestart, None);
    assert_eq!(entry.min_uptime_ms, None);
    assert_eq!(entry.max_restarts, None);
    assert_eq!(entry.restart_delay_ms, None);
    assert!(entry.sandbox.is_none(), "got: {:?}", entry.sandbox);
}

#[test]
fn parse_apps_file_reads_every_declared_field() {
    let apps = parse_apps_file(&full_yaml()).expect("should parse");
    let entry = apps.apps.first().expect("one entry");
    assert_eq!(entry.args, vec!["server.js", "--port=8080"]);
    assert_eq!(entry.depends_on, vec!["db"]);
    assert_eq!(entry.autorestart, Some(false));
    assert_eq!(entry.min_uptime_ms, Some(250));
    assert_eq!(entry.max_restarts, Some(3));
    assert_eq!(entry.restart_delay_ms, Some(40));
}

#[test]
fn parse_apps_file_reads_the_sandbox_section() {
    let apps = parse_apps_file(&full_yaml()).expect("should parse");
    let sandbox = apps
        .apps
        .first()
        .and_then(|entry| entry.sandbox.as_ref())
        .expect("sandbox section");
    assert_eq!(
        sandbox.mode.as_deref(),
        Some(SandboxMode::ReadOnly.as_str())
    );
    assert_eq!(sandbox.network, Some(true));
    assert_eq!(sandbox.writable_roots, Some(Vec::new()));
}

#[test]
fn parse_apps_file_rejects_broken_yaml() {
    let err = parse_apps_file("{{invalid yaml").unwrap_err().to_string();
    assert!(err.contains("cannot parse apps file"), "got: {err}");
}

#[test]
fn parse_apps_file_rejects_a_document_without_the_apps_key() {
    assert!(parse_apps_file("other: 1\n").is_err());
}

#[tokio::test]
async fn load_apps_file_reads_a_file_from_disk() {
    let (_dir, path) = write_apps_file(&minimal_yaml());
    let apps = load_apps_file(&path).await.expect("should load");
    assert_eq!(apps.apps.len(), 1);
}

#[tokio::test]
async fn load_apps_file_reports_a_missing_file() {
    let err = load_apps_file("/nonexistent/apps.yaml")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot read apps file"), "got: {err}");
}

#[tokio::test]
async fn load_apps_file_substitutes_environment_placeholders() {
    let yaml = minimal_yaml().replace(CWD, "${PM3_TEST_APPS_CWD:-/srv/from-default}");
    let (_dir, path) = write_apps_file(&yaml);
    let apps = load_apps_file(&path).await.expect("should load");
    let entry = apps.apps.first().expect("one entry");
    assert_eq!(entry.cwd.as_deref(), Some("/srv/from-default"));
}

#[tokio::test]
async fn load_apps_file_reports_an_unresolvable_placeholder() {
    let yaml = minimal_yaml().replace(CWD, "${PM3_TEST_APPS_UNSET_CWD}");
    let (_dir, path) = write_apps_file(&yaml);
    let err = load_apps_file(&path).await.unwrap_err().to_string();
    assert!(err.contains("PM3_TEST_APPS_UNSET_CWD"), "got: {err}");
}

#[test]
fn parse_apps_file_rejects_an_empty_apps_list() {
    let err = parse_apps_file("apps: []").unwrap_err().to_string();
    assert!(err.contains("no apps"), "got: {err}");
}

#[test]
fn parse_apps_file_rejects_a_duplicate_app_name() {
    let yaml = format!("{}{}", minimal_yaml(), second_app_section(APP_NAME));
    let err = parse_apps_file(&yaml).unwrap_err().to_string();
    assert!(err.contains("duplicate app name 'web'"), "got: {err}");
}

#[test]
fn resolve_checked_rejects_an_invalid_spec() {
    let entry = AppEntry {
        cwd: Some("relative/path".to_string()),
        ..minimal_entry()
    };
    let err = resolve_one_err(&defaults(), &entry);
    assert!(err.contains("relative cwd"), "got: {err}");
}

#[test]
fn parse_apps_file_keeps_the_declared_order() {
    let yaml = format!("{}{}", minimal_yaml(), second_app_section("db"));
    let apps = parse_apps_file(&yaml).expect("should parse");
    let names: Vec<&str> = apps.apps.iter().map(|entry| entry.name.as_str()).collect();
    assert_eq!(names, vec![APP_NAME, "db"]);
}

#[test]
fn resolve_specs_copies_the_script_and_command_line() {
    let entry = AppEntry {
        args: vec!["server.js".to_string()],
        depends_on: vec!["db".to_string()],
        ..minimal_entry()
    };
    let spec = resolve_one(&defaults(), &entry);
    assert_eq!(spec.script, SCRIPT);
    assert_eq!(spec.args, vec!["server.js"]);
    assert_eq!(spec.depends_on, vec!["db"]);
}

#[test]
fn a_resolved_spec_starts_with_no_environment() {
    let spec = resolve_one(&defaults(), &minimal_entry());
    assert!(
        spec.env.is_empty(),
        "the environment arrives from the sidecar file, not from yaml"
    );
}

#[test]
fn an_environment_section_in_a_service_file_is_refused() {
    let entry = AppEntry {
        rejected_env: Some(BTreeMap::from([(
            "TUNNEL_TOKEN".to_string(),
            "eyJhIjoiZjQ2".to_string(),
        )])),
        ..minimal_entry()
    };
    let refused = resolve_one_err(&defaults(), &entry);
    assert_eq!(
        refused,
        "cannot accept 'env' in the declaration for app 'web': move the environment values to 'web.env' beside the service file, so secrets never land in a yaml file"
    );
}

#[test]
fn an_environment_section_in_an_apps_file_is_refused() {
    let yaml = format!(
        "{}    env:\n      TUNNEL_TOKEN: \"eyJhIjoiZjQ2\"\n",
        minimal_yaml()
    );
    let refused = parse_apps_file(&yaml)
        .expect_err("an apps file may not declare an environment")
        .to_string();
    assert!(refused.contains("'web.env'"), "{refused}");
    assert!(!refused.contains("eyJhIjoiZjQ2"), "{refused}");
}

#[test]
fn resolve_specs_fills_the_restart_defaults_from_the_config() {
    let RestartConfig {
        autorestart,
        min_uptime_ms,
        max_restarts,
        restart_delay_ms,
        max_restart_delay_ms,
    } = pm3_config(SandboxMode::WorkspaceWrite.as_str()).restart;
    let spec = resolve_one(&defaults(), &minimal_entry());
    assert_eq!(
        spec.restart_policy(),
        RestartPolicy {
            autorestart,
            min_uptime_ms,
            max_restarts,
            restart_delay_ms,
            max_restart_delay_ms,
        }
    );
}

#[test]
fn resolve_specs_keeps_explicit_restart_overrides() {
    let entry = AppEntry {
        autorestart: Some(false),
        min_uptime_ms: Some(250),
        max_restarts: Some(3),
        restart_delay_ms: Some(40),
        max_restart_delay_ms: Some(9000),
        ..minimal_entry()
    };
    let spec = resolve_one(&defaults(), &entry);
    assert_eq!(
        spec.restart_policy(),
        RestartPolicy {
            autorestart: false,
            min_uptime_ms: 250,
            max_restarts: 3,
            restart_delay_ms: 40,
            max_restart_delay_ms: 9000,
        }
    );
}

#[test]
fn resolve_specs_takes_the_sandbox_mode_from_the_config() {
    let spec = resolve_one(&defaults(), &minimal_entry());
    assert_eq!(spec.sandbox.mode, SandboxMode::WorkspaceWrite);
}

#[test]
fn resolve_specs_takes_the_sandbox_network_flag_from_the_config() {
    let spec = resolve_one(&defaults(), &minimal_entry());
    assert!(!spec.sandbox.network);
}

#[test]
fn resolve_specs_honours_an_explicit_sandbox_mode() {
    let entry = AppEntry {
        sandbox: Some(SandboxEntry {
            mode: Some(SandboxMode::DangerFullAccess.as_str().to_string()),
            ..sandbox_entry()
        }),
        ..minimal_entry()
    };
    let spec = resolve_one(&defaults(), &entry);
    assert_eq!(spec.sandbox.mode, SandboxMode::DangerFullAccess);
}

#[test]
fn resolve_specs_honours_an_explicit_sandbox_network_flag() {
    let entry = AppEntry {
        sandbox: Some(SandboxEntry {
            network: Some(true),
            ..sandbox_entry()
        }),
        ..minimal_entry()
    };
    let spec = resolve_one(&defaults(), &entry);
    assert!(spec.sandbox.network);
}

#[test]
fn resolve_specs_rejects_an_unknown_sandbox_mode() {
    let entry = AppEntry {
        sandbox: Some(SandboxEntry {
            mode: Some("yolo".to_string()),
            ..sandbox_entry()
        }),
        ..minimal_entry()
    };
    let err = resolve_one_err(&defaults(), &entry);
    assert!(
        err.contains("sandbox mode 'yolo' for app 'web'"),
        "got: {err}"
    );
}

#[test]
fn resolve_specs_grants_the_cwd_the_logs_dir_and_the_tmp_dir_by_default() {
    let spec = resolve_one(&defaults(), &minimal_entry());
    assert_eq!(spec.sandbox.derived_roots, vec![CWD, LOGS_DIR, TMP_DIR]);
}

#[test]
fn resolve_specs_keeps_pm3_derived_roots_out_of_the_declared_ones() {
    let spec = resolve_one(&defaults(), &minimal_entry());
    assert!(
        spec.sandbox.writable_roots.is_empty(),
        "roots pm3 derives itself must not read as operator configuration: {:?}",
        spec.sandbox.writable_roots
    );
}

#[test]
fn resolve_specs_drops_a_duplicate_default_writable_root() {
    let entry = AppEntry {
        cwd: Some(LOGS_DIR.to_string()),
        ..minimal_entry()
    };
    let spec = resolve_one(&defaults(), &entry);
    assert_eq!(spec.sandbox.derived_roots, vec![LOGS_DIR, TMP_DIR]);
}

#[test]
fn resolve_specs_skips_a_missing_tmp_dir() {
    let defaults = SpecDefaults {
        tmp_dir: None,
        ..defaults()
    };
    let spec = resolve_one(&defaults, &minimal_entry());
    assert_eq!(spec.sandbox.derived_roots, vec![CWD, LOGS_DIR]);
}

#[test]
fn resolve_specs_skips_a_blank_tmp_dir() {
    let defaults = SpecDefaults {
        tmp_dir: Some(""),
        ..defaults()
    };
    let spec = resolve_one(&defaults, &minimal_entry());
    assert_eq!(spec.sandbox.derived_roots, vec![CWD, LOGS_DIR]);
}

#[test]
fn resolve_specs_grants_no_writable_root_in_read_only_mode() {
    let defaults = SpecDefaults {
        sandbox_mode: SandboxMode::ReadOnly,
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
