use usecases::SandboxMode;

use super::*;
use crate::{
    SpecDefaults,
    config_sections::{pm3_section, telemetry_section},
    load_service_file, parse_config, parse_service_file, resolve_checked,
};

const NAME: &str = "mihomo-rule";
const PROGRAM: &str = "/opt/homebrew/bin/mihomo";
const INSTALLED_PROGRAM: &str = "/bin/sh";
const CWD: &str = "/home/dev/.pm3/mihomo-rule";

const HOME: &str = "/home/dev";

fn request<'r>(env: &'r [String], writable_dirs: &'r [String]) -> InlineRequest<'r> {
    InlineRequest {
        name: NAME,
        program: PROGRAM,
        args: &[],
        cwd: Some(CWD),
        home: Some(HOME),
        env,
        cron: None,
        autorestart: None,
        network: true,
        writable_dirs,
    }
}

#[test]
fn an_inline_request_becomes_a_single_app() {
    let entry = inline_entry(&request(&[], &[])).expect("the request should resolve");
    assert_eq!(entry.name, NAME);
    assert_eq!(entry.script, PROGRAM);
    assert_eq!(entry.cwd.as_deref(), Some("${HOME}/.pm3/mihomo-rule"));
}

#[test]
fn a_program_under_the_home_folds_into_a_placeholder() {
    let mut asked = request(&[], &[]);
    asked.program = "/home/dev/bin/mihomo";
    let entry = inline_entry(&asked).expect("the request should resolve");
    assert_eq!(entry.script, "${HOME}/bin/mihomo");
}

#[test]
fn the_program_arguments_are_carried_verbatim() {
    let args = ["-d".to_string(), CWD.to_string(), "-f".to_string()];
    let mut asked = request(&[], &[]);
    asked.args = &args;
    let entry = inline_entry(&asked).expect("the request should resolve");
    assert_eq!(entry.args, ["-d", "${HOME}/.pm3/mihomo-rule", "-f"]);
}

#[test]
fn a_bare_service_cwd_token_is_stored_braced() {
    let args = ["-d".to_string(), "PM3_SERVICE_CWD".to_string()];
    let mut asked = request(&[], &[]);
    asked.args = &args;
    let entry = inline_entry(&asked).expect("the request should resolve");
    assert_eq!(entry.args, ["-d", "${PM3_SERVICE_CWD}"]);
}

#[test]
fn the_network_switch_reaches_the_sandbox_section() {
    let entry = inline_entry(&request(&[], &[])).expect("the request should resolve");
    let sandbox = entry
        .sandbox
        .as_ref()
        .expect("an inline app always declares a sandbox");
    assert_eq!(sandbox.network, Some(true));
}

#[test]
fn no_network_switch_leaves_the_configured_default_alone() {
    let mut asked = request(&[], &[]);
    asked.network = false;
    let entry = inline_entry(&asked).expect("the request should resolve");
    let sandbox = entry
        .sandbox
        .as_ref()
        .expect("an inline app always declares a sandbox");
    assert_eq!(sandbox.network, None);
}

#[test]
fn an_inline_app_never_asks_for_a_sandbox_mode() {
    let entry = inline_entry(&request(&[], &[])).expect("the request should resolve");
    let sandbox = entry
        .sandbox
        .as_ref()
        .expect("an inline app always declares a sandbox");
    assert_eq!(sandbox.mode, None);
}

#[test]
fn no_writable_dirs_leaves_the_defaults_alone() {
    let entry = inline_entry(&request(&[], &[])).expect("the request should resolve");
    let sandbox = entry
        .sandbox
        .as_ref()
        .expect("an inline app always declares a sandbox");
    assert_eq!(sandbox.writable_roots, None);
}

#[test]
fn writable_dirs_are_declared_on_their_own() {
    let dirs = ["/srv/data".to_string()];
    let entry = inline_entry(&request(&[], &dirs)).expect("the request should resolve");
    let sandbox = entry
        .sandbox
        .as_ref()
        .expect("an inline app always declares a sandbox");
    assert_eq!(
        sandbox.writable_roots,
        Some(vec!["/srv/data".to_string()]),
        "the working directory is derived at resolve time, not declared here"
    );
}

#[test]
fn a_writable_dir_equal_to_the_working_directory_is_not_repeated() {
    let dirs = [CWD.to_string()];
    let entry = inline_entry(&request(&[], &dirs)).expect("the request should resolve");
    let sandbox = entry
        .sandbox
        .as_ref()
        .expect("an inline app always declares a sandbox");
    assert_eq!(
        sandbox.writable_roots,
        Some(vec!["${HOME}/.pm3/mihomo-rule".to_string()])
    );
}

#[test]
fn environment_pairs_are_split_on_the_first_equals_sign() {
    let env = ["PATH=/usr/bin".to_string(), "GREETING=a=b".to_string()];
    let entry = inline_entry(&request(&env, &[])).expect("the request should resolve");
    assert_eq!(entry.env.get("PATH").map(String::as_str), Some("/usr/bin"));
    assert_eq!(entry.env.get("GREETING").map(String::as_str), Some("a=b"));
}

#[test]
fn an_environment_value_under_the_home_folds_back_to_the_placeholder() {
    let env = [format!("DATA={HOME}/data")];
    let entry = inline_entry(&request(&env, &[])).expect("the request should resolve");
    assert_eq!(
        entry.env.get("DATA").map(String::as_str),
        Some("${HOME}/data"),
        "both encoders must agree, or reconcile rejects the very next apps file"
    );
}

#[test]
fn an_environment_entry_without_an_equals_sign_is_rejected() {
    let env = ["JUST_A_KEY".to_string()];
    let err = inline_entry(&request(&env, &[])).unwrap_err().to_string();
    assert_eq!(
        err,
        "cannot accept environment entry 'JUST_A_KEY': expected KEY=VALUE"
    );
}

#[test]
fn an_environment_entry_without_a_key_is_rejected() {
    let env = ["=orphan".to_string()];
    let err = inline_entry(&request(&env, &[])).unwrap_err().to_string();
    assert!(err.contains("expected KEY=VALUE"), "got: {err}");
}

#[test]
fn an_encoded_inline_app_reads_back_unchanged() {
    let env = ["PATH=/usr/bin:/bin".to_string()];
    let dirs = ["/srv/data".to_string()];
    let entry = inline_entry(&request(&env, &dirs)).expect("the request should resolve");
    let yaml = encode_service_file(&entry);
    let reparsed = parse_service_file(&yaml).expect("the encoded app should parse");
    assert_eq!(reparsed.name, NAME);
    assert_eq!(reparsed.env, entry.env);
}

#[test]
fn an_encoded_app_with_no_collections_still_reads_back() {
    let entry = inline_entry(&request(&[], &[])).expect("the request should resolve");
    let yaml = encode_service_file(&entry);
    assert!(
        !yaml.contains('~'),
        "empty collections must be omitted: {yaml}"
    );
    let reparsed = parse_service_file(&yaml).expect("the encoded app should parse");
    assert!(reparsed.args.is_empty());
    assert!(reparsed.env.is_empty());
}

#[test]
fn an_encoded_inline_app_resolves_into_a_spec() {
    let yaml = format!(
        "{}{}",
        pm3_section(
            "/tmp/pm3-fixture",
            1600,
            SandboxMode::WorkspaceWrite.as_str()
        ),
        telemetry_section("info")
    );
    let config = parse_config(&yaml).expect("the fixture config should parse");
    let defaults = SpecDefaults::from_config(
        &config.pm3,
        "/tmp/pm3-fixture",
        "/tmp/pm3-fixture/logs",
        None,
    )
    .expect("the fixture defaults should build");
    let mut asked = request(&[], &[]);
    asked.cwd = None;
    asked.program = INSTALLED_PROGRAM;
    let entry = inline_entry(&asked).expect("the request should resolve");
    let specs = [resolve_checked(&defaults, &entry).expect("the inline app should resolve")];
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].cwd, "/tmp/pm3-fixture/mihomo-rule");
    assert_eq!(specs[0].script, INSTALLED_PROGRAM);
}

#[tokio::test]
async fn a_home_placeholder_expands_when_the_config_file_is_loaded() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("mihomo-rule.yaml");
    let mut asked = request(&[], &[]);
    asked.cwd = Some("/home/dev/work");
    let entry = inline_entry(&asked).expect("the request should resolve");
    std::fs::write(&path, encode_service_file(&entry)).expect("write the config file");
    let loaded = load_service_file(&path.to_string_lossy())
        .await
        .expect("the config file should load");
    let expected = format!(
        "{}/work",
        std::env::var("HOME").expect("tests always run with HOME")
    );
    assert_eq!(loaded.cwd.as_deref(), Some(expected.as_str()));
}

#[test]
fn identical_text_has_no_diff() {
    assert!(diff_lines("a\nb\n", "a\nb\n").is_empty());
}

#[test]
fn a_changed_line_shows_both_sides() {
    assert_eq!(
        diff_lines("a\nb\n", "a\nc\n"),
        ["-b".to_string(), "+c".to_string()]
    );
}

#[test]
fn a_removed_line_shows_only_the_old_side() {
    assert_eq!(diff_lines("a\nb\n", "a\n"), ["-b".to_string()]);
}

#[test]
fn an_added_line_shows_only_the_new_side() {
    assert_eq!(diff_lines("a\n", "a\nb\n"), ["+b".to_string()]);
}

fn fully_declared_entry() -> AppEntry {
    AppEntry {
        name: NAME.to_string(),
        script: PROGRAM.to_string(),
        cwd: Some(CWD.to_string()),
        args: vec!["-d".to_string()],
        env: std::collections::BTreeMap::new(),
        depends_on: vec!["db".to_string()],
        autorestart: Some(false),
        min_uptime_ms: Some(2000),
        max_restarts: Some(7),
        restart_delay_ms: Some(50),
        schedule: None,
        sandbox: Some(SandboxEntry {
            mode: Some(SandboxMode::WorkspaceWrite.as_str().to_string()),
            network: Some(false),
            writable_roots: Some(vec!["/srv".to_string()]),
        }),
    }
}

#[test]
fn a_fully_declared_app_encodes_every_field() {
    let yaml = encode_service_file(&fully_declared_entry());
    for needle in [
        "depends_on:",
        "autorestart: false",
        "min_uptime_ms: 2000",
        "max_restarts: 7",
        "restart_delay_ms: 50",
        "mode: \"workspace-write\"",
        "network: false",
        "writable_roots:",
    ] {
        assert!(yaml.contains(needle), "{needle} missing from: {yaml}");
    }
}

#[test]
fn a_fully_declared_app_reads_back_unchanged() {
    let encoded = encode_service_file(&fully_declared_entry());
    let reparsed = parse_service_file(&encoded).expect("the encoded app should parse");
    assert_eq!(reparsed.depends_on, ["db"]);
    assert_eq!(reparsed.autorestart, Some(false));
    assert_eq!(reparsed.max_restarts, Some(7));
}

#[test]
fn an_empty_sandbox_section_is_omitted() {
    let mut entry = fully_declared_entry();
    entry.sandbox = Some(SandboxEntry {
        mode: None,
        network: None,
        writable_roots: None,
    });
    let yaml = encode_service_file(&entry);
    assert!(!yaml.contains("sandbox"), "got: {yaml}");
}

#[test]
fn an_app_without_a_sandbox_section_is_encoded_plainly() {
    let mut entry = fully_declared_entry();
    entry.sandbox = None;
    let yaml = encode_service_file(&entry);
    assert!(!yaml.contains("sandbox"), "got: {yaml}");
}

#[test]
fn control_characters_in_environment_values_survive_a_round_trip() {
    let env = [
        "MULTILINE=line one\nline two".to_string(),
        "TABBED=a\tb".to_string(),
        "CARRIAGE=a\rb".to_string(),
        "BELL=a\u{7}b".to_string(),
        "MIXED=quote\"back\\slash\nnewline".to_string(),
    ];
    let entry = inline_entry(&request(&env, &[])).expect("the request should resolve");
    let yaml = encode_service_file(&entry);
    let reparsed = parse_service_file(&yaml).expect("the encoded app should parse");
    assert_eq!(reparsed.env, entry.env);
}

#[test]
fn a_value_with_a_newline_is_encoded_on_a_single_line() {
    let env = ["MULTILINE=line one\nline two".to_string()];
    let entry = inline_entry(&request(&env, &[])).expect("the request should resolve");
    let yaml = encode_service_file(&entry);
    assert!(
        yaml.contains(r#""MULTILINE": "line one\nline two""#),
        "got: {yaml}"
    );
}

#[test]
fn an_inline_cron_reaches_the_encoded_file() {
    let env: Vec<String> = Vec::new();
    let dirs: Vec<String> = Vec::new();
    let mut ask = request(&env, &dirs);
    ask.cron = Some("~ * * * *");
    ask.autorestart = Some(false);
    let entry = inline_entry(&ask).expect("inline request should build");
    let yaml = encode_service_file(&entry);
    assert!(yaml.contains(r#"schedule: "~ * * * *""#), "got: {yaml}");
    let reparsed = parse_service_file(&yaml).expect("the encoded app should parse");
    assert_eq!(reparsed.schedule.as_deref(), Some("~ * * * *"));
    assert_eq!(reparsed.autorestart, Some(false));
}

#[test]
fn an_app_without_a_cron_omits_the_schedule_key() {
    let env: Vec<String> = Vec::new();
    let dirs: Vec<String> = Vec::new();
    let entry = inline_entry(&request(&env, &dirs)).expect("inline request should build");
    assert!(!encode_service_file(&entry).contains("schedule:"));
}
