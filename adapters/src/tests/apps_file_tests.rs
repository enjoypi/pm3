use usecases::RestartPolicy;

use super::{test_helpers::*, *};
use crate::config::{
    SANDBOX_MODE_DANGER_FULL_ACCESS, SANDBOX_MODE_READ_ONLY, SANDBOX_MODE_WORKSPACE_WRITE,
};

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
    assert!(entry.env.is_empty(), "got: {:?}", entry.env);
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
    assert_eq!(entry.env.get("PORT").map(String::as_str), Some("8080"));
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
    assert_eq!(sandbox.mode.as_deref(), Some(SANDBOX_MODE_READ_ONLY));
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

#[test]
fn load_apps_file_reads_a_file_from_disk() {
    let (_dir, path) = write_apps_file(&minimal_yaml());
    let apps = load_apps_file(&path).expect("should load");
    assert_eq!(apps.apps.len(), 1);
}

#[test]
fn load_apps_file_reports_a_missing_file() {
    let err = load_apps_file("/nonexistent/apps.yaml")
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot read apps file"), "got: {err}");
}

#[test]
fn load_apps_file_substitutes_environment_placeholders() {
    let yaml = minimal_yaml().replace(CWD, "${PM3_TEST_APPS_CWD:-/srv/from-default}");
    let (_dir, path) = write_apps_file(&yaml);
    let apps = load_apps_file(&path).expect("should load");
    let entry = apps.apps.first().expect("one entry");
    assert_eq!(entry.cwd.as_deref(), Some("/srv/from-default"));
}

#[test]
fn load_apps_file_reports_an_unresolvable_placeholder() {
    let yaml = minimal_yaml().replace(CWD, "${PM3_TEST_APPS_UNSET_CWD}");
    let (_dir, path) = write_apps_file(&yaml);
    let err = load_apps_file(&path).unwrap_err().to_string();
    assert!(err.contains("PM3_TEST_APPS_UNSET_CWD"), "got: {err}");
}

#[test]
fn resolve_specs_rejects_an_empty_apps_list() {
    let err = resolve_specs(&defaults(), &apps_of(Vec::new()))
        .unwrap_err()
        .to_string();
    assert!(err.contains("no apps"), "got: {err}");
}

#[test]
fn resolve_specs_rejects_a_duplicate_app_name() {
    let apps = apps_of(vec![minimal_entry(), minimal_entry()]);
    let err = resolve_specs(&defaults(), &apps).unwrap_err().to_string();
    assert!(err.contains("duplicate app name 'web'"), "got: {err}");
}

#[test]
fn resolve_specs_rejects_an_invalid_spec() {
    let entry = AppEntry {
        cwd: Some("relative/path".to_string()),
        ..minimal_entry()
    };
    let err = resolve_one_err(&defaults(), entry);
    assert!(err.contains("relative cwd"), "got: {err}");
}

#[test]
fn resolve_specs_keeps_the_declared_order() {
    let second = AppEntry {
        name: "db".to_string(),
        ..minimal_entry()
    };
    let apps = apps_of(vec![minimal_entry(), second]);
    let specs = resolve_specs(&defaults(), &apps).expect("should resolve");
    let names: Vec<&str> = specs.iter().map(|spec| spec.name.as_str()).collect();
    assert_eq!(names, vec![APP_NAME, "db"]);
}

#[test]
fn resolve_specs_copies_the_script_and_command_line() {
    let entry = AppEntry {
        args: vec!["server.js".to_string()],
        depends_on: vec!["db".to_string()],
        ..minimal_entry()
    };
    let spec = resolve_one(&defaults(), entry);
    assert_eq!(spec.script, SCRIPT);
    assert_eq!(spec.args, vec!["server.js"]);
    assert_eq!(spec.depends_on, vec!["db"]);
}

#[test]
fn resolve_specs_flattens_the_environment_in_key_order() {
    let entry = AppEntry {
        env: BTreeMap::from([
            ("PORT".to_string(), "8080".to_string()),
            ("HOST".to_string(), "0.0.0.0".to_string()),
        ]),
        ..minimal_entry()
    };
    let spec = resolve_one(&defaults(), entry);
    assert_eq!(
        spec.env,
        vec![
            ("HOST".to_string(), "0.0.0.0".to_string()),
            ("PORT".to_string(), "8080".to_string()),
        ]
    );
}

#[test]
fn resolve_specs_fills_the_restart_defaults_from_the_config() {
    let RestartConfig {
        min_uptime_ms,
        max_restarts,
        restart_delay_ms,
    } = pm3_config(SANDBOX_MODE_WORKSPACE_WRITE).restart;
    let spec = resolve_one(&defaults(), minimal_entry());
    assert_eq!(
        spec.restart_policy(),
        RestartPolicy {
            autorestart: true,
            min_uptime_ms,
            max_restarts,
            restart_delay_ms,
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
        ..minimal_entry()
    };
    let spec = resolve_one(&defaults(), entry);
    assert_eq!(
        spec.restart_policy(),
        RestartPolicy {
            autorestart: false,
            min_uptime_ms: 250,
            max_restarts: 3,
            restart_delay_ms: 40,
        }
    );
}

#[test]
fn resolve_specs_takes_the_sandbox_mode_from_the_config() {
    let spec = resolve_one(&defaults(), minimal_entry());
    assert_eq!(spec.sandbox.mode, SandboxMode::WorkspaceWrite);
}

#[test]
fn resolve_specs_takes_the_sandbox_network_flag_from_the_config() {
    let spec = resolve_one(&defaults(), minimal_entry());
    assert!(!spec.sandbox.network);
}

#[test]
fn resolve_specs_honours_an_explicit_sandbox_mode() {
    let entry = AppEntry {
        sandbox: Some(SandboxEntry {
            mode: Some(SANDBOX_MODE_DANGER_FULL_ACCESS.to_string()),
            ..sandbox_entry()
        }),
        ..minimal_entry()
    };
    let spec = resolve_one(&defaults(), entry);
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
    let spec = resolve_one(&defaults(), entry);
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
    let err = resolve_one_err(&defaults(), entry);
    assert!(
        err.contains("sandbox mode 'yolo' for app 'web'"),
        "got: {err}"
    );
}

#[test]
fn resolve_specs_grants_the_cwd_the_logs_dir_and_the_tmp_dir_by_default() {
    let spec = resolve_one(&defaults(), minimal_entry());
    assert_eq!(spec.sandbox.writable_roots, vec![CWD, LOGS_DIR, TMP_DIR]);
}

#[test]
fn resolve_specs_drops_a_duplicate_default_writable_root() {
    let entry = AppEntry {
        cwd: Some(LOGS_DIR.to_string()),
        ..minimal_entry()
    };
    let spec = resolve_one(&defaults(), entry);
    assert_eq!(spec.sandbox.writable_roots, vec![LOGS_DIR, TMP_DIR]);
}

#[test]
fn resolve_specs_skips_a_missing_tmp_dir() {
    let defaults = SpecDefaults {
        tmp_dir: None,
        ..defaults()
    };
    let spec = resolve_one(&defaults, minimal_entry());
    assert_eq!(spec.sandbox.writable_roots, vec![CWD, LOGS_DIR]);
}

#[test]
fn resolve_specs_skips_a_blank_tmp_dir() {
    let defaults = SpecDefaults {
        tmp_dir: Some(""),
        ..defaults()
    };
    let spec = resolve_one(&defaults, minimal_entry());
    assert_eq!(spec.sandbox.writable_roots, vec![CWD, LOGS_DIR]);
}

#[test]
fn resolve_specs_grants_no_writable_root_in_read_only_mode() {
    let defaults = SpecDefaults {
        sandbox_mode: SandboxMode::ReadOnly,
        ..defaults()
    };
    let spec = resolve_one(&defaults, minimal_entry());
    assert!(
        spec.sandbox.writable_roots.is_empty(),
        "got: {:?}",
        spec.sandbox.writable_roots
    );
}

#[test]
fn resolve_specs_grants_no_writable_root_in_full_access_mode() {
    let defaults = SpecDefaults {
        sandbox_mode: SandboxMode::DangerFullAccess,
        ..defaults()
    };
    let spec = resolve_one(&defaults, minimal_entry());
    assert!(
        spec.sandbox.writable_roots.is_empty(),
        "got: {:?}",
        spec.sandbox.writable_roots
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
    let spec = resolve_one(&defaults(), entry);
    assert_eq!(spec.sandbox.writable_roots, vec!["/srv/web/var"]);
}

#[test]
fn resolve_specs_honours_an_explicitly_empty_writable_roots_list() {
    let entry = AppEntry {
        sandbox: Some(SandboxEntry {
            writable_roots: Some(Vec::new()),
            ..sandbox_entry()
        }),
        ..minimal_entry()
    };
    let spec = resolve_one(&defaults(), entry);
    assert!(
        spec.sandbox.writable_roots.is_empty(),
        "got: {:?}",
        spec.sandbox.writable_roots
    );
}

#[test]
fn spec_defaults_reads_the_pm3_sandbox_section() {
    let mut pm3 = pm3_config(SANDBOX_MODE_WORKSPACE_WRITE);
    pm3.sandbox.network = true;
    let defaults =
        SpecDefaults::from_config(&pm3, HOME_DIR, LOGS_DIR, Some(TMP_DIR)).expect("should build");
    assert_eq!(defaults.sandbox_mode, SandboxMode::WorkspaceWrite);
    assert!(defaults.sandbox_network);
    assert_eq!(defaults.restart.max_restarts, pm3.restart.max_restarts);
}

#[test]
fn spec_defaults_rejects_an_unknown_configured_sandbox_mode() {
    let mut pm3 = pm3_config(SANDBOX_MODE_WORKSPACE_WRITE);
    pm3.sandbox.mode = "yolo".to_string();
    let err = SpecDefaults::from_config(&pm3, HOME_DIR, LOGS_DIR, Some(TMP_DIR))
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
    let apps = AppsFile { apps: vec![entry] };
    let err = resolve_specs(&defaults(), &apps).unwrap_err().to_string();
    assert_eq!(
        err,
        "cannot find 'pm3-not-a-real-program' for app 'web' on pm3.search_path"
    );
}
