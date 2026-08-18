use usecases::{ReadScope, SandboxMode};

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

fn request(writable_dirs: &[String]) -> InlineRequest<'_> {
    InlineRequest {
        name: NAME,
        program: PROGRAM,
        args: &[],
        cwd: Some(CWD),
        home: Some(HOME),
        cron: None,
        autorestart: None,
        network: true,
        writable_dirs,
        readable_dirs: &[],
        max_memory: None,
        ready_exec: &[],
        ready_tcp: None,
        listen_timeout_ms: None,
        stop_exit_codes: &[],
    }
}

#[test]
fn an_inline_request_becomes_a_single_app() {
    let entry = inline_entry(&request(&[]));
    assert_eq!(entry.name, NAME);
    assert_eq!(entry.script, PROGRAM);
    assert_eq!(entry.cwd.as_deref(), Some("${HOME}/.pm3/mihomo-rule"));
}

#[test]
fn a_program_under_the_home_folds_into_a_placeholder() {
    let mut asked = request(&[]);
    asked.program = "/home/dev/bin/mihomo";
    let entry = inline_entry(&asked);
    assert_eq!(entry.script, "${HOME}/bin/mihomo");
}

#[test]
fn the_program_arguments_are_carried_verbatim() {
    let args = ["-d".to_string(), CWD.to_string(), "-f".to_string()];
    let mut asked = request(&[]);
    asked.args = &args;
    let entry = inline_entry(&asked);
    assert_eq!(entry.args, ["-d", "${HOME}/.pm3/mihomo-rule", "-f"]);
}

#[test]
fn a_bare_service_cwd_token_is_stored_braced() {
    let args = ["-d".to_string(), "PM3_SERVICE_CWD".to_string()];
    let mut asked = request(&[]);
    asked.args = &args;
    let entry = inline_entry(&asked);
    assert_eq!(entry.args, ["-d", "${PM3_SERVICE_CWD}"]);
}

#[test]
fn the_network_switch_reaches_the_sandbox_section() {
    let entry = inline_entry(&request(&[]));
    let sandbox = entry
        .sandbox
        .as_ref()
        .expect("an inline app always declares a sandbox");
    assert_eq!(sandbox.network, Some(true));
}

#[test]
fn no_network_switch_leaves_the_configured_default_alone() {
    let mut asked = request(&[]);
    asked.network = false;
    let entry = inline_entry(&asked);
    let sandbox = entry
        .sandbox
        .as_ref()
        .expect("an inline app always declares a sandbox");
    assert_eq!(sandbox.network, None);
}

#[test]
fn an_inline_app_never_asks_for_a_sandbox_mode() {
    let entry = inline_entry(&request(&[]));
    let sandbox = entry
        .sandbox
        .as_ref()
        .expect("an inline app always declares a sandbox");
    assert_eq!(sandbox.mode, None);
}

#[test]
fn no_writable_dirs_leaves_the_defaults_alone() {
    let entry = inline_entry(&request(&[]));
    let sandbox = entry
        .sandbox
        .as_ref()
        .expect("an inline app always declares a sandbox");
    assert_eq!(sandbox.writable_roots, None);
}

#[test]
fn writable_dirs_are_declared_on_their_own() {
    let dirs = ["/srv/data".to_string()];
    let entry = inline_entry(&request(&dirs));
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
    let entry = inline_entry(&request(&dirs));
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
fn an_inline_app_never_declares_an_environment() {
    let entry = inline_entry(&request(&[]));
    assert!(entry.rejected_env.is_none());
    assert!(
        !encode_service_file(&entry).contains("env:"),
        "environment values belong in the sidecar file"
    );
}

#[test]
fn an_encoded_inline_app_reads_back_unchanged() {
    let dirs = ["/srv/data".to_string()];
    let entry = inline_entry(&request(&dirs));
    let yaml = encode_service_file(&entry);
    let reparsed = parse_service_file(&yaml).expect("the encoded app should parse");
    assert_eq!(reparsed.name, NAME);
    assert_eq!(reparsed.cwd, entry.cwd);
}

#[test]
fn an_encoded_app_with_no_collections_still_reads_back() {
    let entry = inline_entry(&request(&[]));
    let yaml = encode_service_file(&entry);
    assert!(
        !yaml.contains('~'),
        "empty collections must be omitted: {yaml}"
    );
    let reparsed = parse_service_file(&yaml).expect("the encoded app should parse");
    assert!(reparsed.args.is_empty());
    assert!(reparsed.rejected_env.is_none());
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
        "/tmp/pm3-fixture-cfg",
        "/tmp/pm3-fixture/logs",
        None,
    )
    .expect("the fixture defaults should build");
    let mut asked = request(&[]);
    asked.cwd = None;
    asked.program = INSTALLED_PROGRAM;
    let entry = inline_entry(&asked);
    let specs = [resolve_checked(&defaults, &entry).expect("the inline app should resolve")];
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].cwd, "/tmp/pm3-fixture/mihomo-rule");
    assert_eq!(specs[0].script, INSTALLED_PROGRAM);
}

#[tokio::test]
async fn a_home_placeholder_expands_when_the_config_file_is_loaded() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("mihomo-rule.yaml");
    let mut asked = request(&[]);
    asked.cwd = Some("/home/dev/work");
    let entry = inline_entry(&asked);
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
        rejected_env: None,
        depends_on: vec!["db".to_string()],
        autorestart: Some(false),
        min_uptime_ms: Some(2000),
        max_restarts: Some(7),
        restart_delay_ms: Some(50),
        max_restart_delay_ms: Some(60000),
        listen_timeout_ms: Some(20000),
        ready_probe: Some(ReadyProbeEntry {
            exec: None,
            tcp: Some("127.0.0.1:8080".to_string()),
        }),
        schedule: None,
        max_memory: Some("300M".to_string()),
        stop_exit_codes: Vec::new(),
        sandbox: Some(SandboxEntry {
            mode: Some(SandboxMode::WorkspaceWrite.as_str().to_string()),
            read: Some(ReadScope::Minimal.as_str().to_string()),
            network: Some(false),
            writable_roots: Some(vec!["/srv".to_string()]),
            readable_roots: Some(vec!["/opt/data".to_string()]),
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
        "max_restart_delay_ms: 60000",
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
        read: None,
        network: None,
        writable_roots: None,
        readable_roots: None,
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
fn control_characters_in_arguments_survive_a_round_trip() {
    let args = [
        "line one\nline two".to_string(),
        "a\tb".to_string(),
        "a\rb".to_string(),
        "a\u{7}b".to_string(),
        "quote\"back\\slash\nnewline".to_string(),
    ];
    let mut asked = request(&[]);
    asked.args = &args;
    let entry = inline_entry(&asked);
    let yaml = encode_service_file(&entry);
    let reparsed = parse_service_file(&yaml).expect("the encoded app should parse");
    assert_eq!(reparsed.args, entry.args);
}

#[tokio::test]
async fn a_dollar_sign_in_an_argument_survives_loading_the_service_file() {
    let args = ["--format".to_string(), "${LEVEL}: $MSG".to_string()];
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("mihomo-rule.yaml");
    let mut asked = request(&[]);
    asked.args = &args;
    let entry = inline_entry(&asked);
    std::fs::write(&path, encode_service_file(&entry)).expect("write the config file");
    let loaded = load_service_file(&path.to_string_lossy())
        .await
        .expect("an argument pm3 wrote itself must load back");
    assert_eq!(loaded.args, args);
}

#[tokio::test]
async fn a_home_placeholder_inside_an_argument_still_expands() {
    let home = std::env::var("HOME").expect("tests always run with HOME");
    let args = [format!("{home}/data"), "$LITERAL".to_string()];
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("mihomo-rule.yaml");
    let mut asked = request(&[]);
    asked.args = &args;
    asked.home = Some(home.as_str());
    let entry = inline_entry(&asked);
    let yaml = encode_service_file(&entry);
    assert!(yaml.contains("${HOME}/data"), "got: {yaml}");
    std::fs::write(&path, &yaml).expect("write the config file");
    let loaded = load_service_file(&path.to_string_lossy())
        .await
        .expect("the config file should load");
    assert_eq!(loaded.args, args);
}

#[test]
fn a_service_cwd_placeholder_is_not_escaped_away() {
    let args = ["${PM3_SERVICE_CWD}/db".to_string()];
    let mut asked = request(&[]);
    asked.args = &args;
    let yaml = encode_service_file(&inline_entry(&asked));
    assert!(yaml.contains("${PM3_SERVICE_CWD}/db"), "got: {yaml}");
}

#[test]
fn an_argument_with_a_newline_is_encoded_on_a_single_line() {
    let args = ["line one\nline two".to_string()];
    let mut asked = request(&[]);
    asked.args = &args;
    let entry = inline_entry(&asked);
    let yaml = encode_service_file(&entry);
    assert!(yaml.contains(r#"- "line one\nline two""#), "got: {yaml}");
}

#[test]
fn an_inline_cron_reaches_the_encoded_file() {
    let dirs: Vec<String> = Vec::new();
    let mut ask = request(&dirs);
    ask.cron = Some("~ * * * *");
    ask.autorestart = Some(false);
    let entry = inline_entry(&ask);
    let yaml = encode_service_file(&entry);
    assert!(yaml.contains(r#"schedule: "~ * * * *""#), "got: {yaml}");
    let reparsed = parse_service_file(&yaml).expect("the encoded app should parse");
    assert_eq!(reparsed.schedule.as_deref(), Some("~ * * * *"));
    assert_eq!(reparsed.autorestart, Some(false));
}

#[test]
fn an_app_without_a_cron_omits_the_schedule_key() {
    let dirs: Vec<String> = Vec::new();
    let entry = inline_entry(&request(&dirs));
    assert!(!encode_service_file(&entry).contains("schedule:"));
}

#[test]
fn readable_dirs_are_declared_and_folded_like_the_writable_ones() {
    let readable = ["/home/dev/data".to_string()];
    let mut asked = request(&[]);
    asked.readable_dirs = &readable;
    let entry = inline_entry(&asked);
    let sandbox = entry
        .sandbox
        .as_ref()
        .expect("an inline app always declares a sandbox");
    assert_eq!(
        sandbox.readable_roots,
        Some(vec!["${HOME}/data".to_string()])
    );
}

#[test]
fn no_readable_dirs_leaves_the_defaults_alone() {
    let entry = inline_entry(&request(&[]));
    let sandbox = entry
        .sandbox
        .as_ref()
        .expect("an inline app always declares a sandbox");
    assert_eq!(sandbox.readable_roots, None);
}

#[test]
fn a_declared_memory_limit_reaches_the_encoded_service() {
    let mut asked = request(&[]);
    asked.max_memory = Some("300M");
    let yaml = encode_service_file(&inline_entry(&asked));
    assert!(yaml.contains("max_memory: \"300M\""), "got: {yaml}");
}

#[test]
fn an_inline_exec_probe_round_trips() {
    let probe = ["curl".to_string(), "-sf".to_string()];
    let mut asked = request(&[]);
    asked.ready_exec = &probe;
    let entry = inline_entry(&asked);
    let yaml = encode_service_file(&entry);
    assert!(yaml.contains("ready_probe:\n"), "{yaml}");
    assert!(yaml.contains("- \"curl\""), "{yaml}");
    let reparsed = parse_service_file(&yaml).expect("the encoded app should parse");
    assert_eq!(
        reparsed.ready_probe.and_then(|section| section.exec),
        Some(probe.to_vec())
    );
}

#[test]
fn an_inline_tcp_probe_round_trips() {
    let mut asked = request(&[]);
    asked.ready_tcp = Some("127.0.0.1:8080");
    let entry = inline_entry(&asked);
    let yaml = encode_service_file(&entry);
    assert!(yaml.contains("tcp: \"127.0.0.1:8080\""), "{yaml}");
    let reparsed = parse_service_file(&yaml).expect("the encoded app should parse");
    assert_eq!(
        reparsed.ready_probe.and_then(|section| section.tcp),
        Some("127.0.0.1:8080".to_string())
    );
}

#[test]
fn an_empty_probe_section_is_omitted() {
    let entry = AppEntry {
        ready_probe: Some(ReadyProbeEntry {
            exec: None,
            tcp: None,
        }),
        ..fully_declared_entry()
    };
    let yaml = encode_service_file(&entry);
    assert!(!yaml.contains("ready_probe"), "{yaml}");
}

#[test]
fn an_inline_request_renders_stop_exit_codes() {
    let entry = inline_entry(&InlineRequest {
        stop_exit_codes: &[0, 3],
        ..request(&[])
    });
    let yaml = encode_service_file(&entry);
    assert!(
        yaml.contains("stop_exit_codes:\n  - 0\n  - 3\n"),
        "got: {yaml}"
    );
    let reparsed = parse_service_file(&yaml).expect("the encoded app should parse");
    assert_eq!(reparsed.stop_exit_codes, vec![0, 3]);
}

#[test]
fn an_inline_request_without_stop_exit_codes_renders_none() {
    let yaml = encode_service_file(&inline_entry(&request(&[])));
    assert!(!yaml.contains("stop_exit_codes"), "got: {yaml}");
}
