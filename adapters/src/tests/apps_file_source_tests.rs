use super::*;
use crate::spec_sources::{
    HOST_HOME, SERVICE_SCRIPT, register_service, service_yaml, spec_source_in, with_global_env,
    write_env_file, write_service_file,
};

struct Fixture {
    dir: tempfile::TempDir,
    source: SpecSource,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().expect("create temp dir");
    let source = spec_source_in(dir.path());
    Fixture { dir, source }
}

#[test]
fn portable_roots_leave_forward_slash_paths_alone() {
    let fixture = fixture();
    let before = fixture.source.home_dir.clone();
    let source = fixture.source.with_portable_roots();
    assert_eq!(source.home_dir, before);
    assert_eq!(source.logs_dir, format!("{before}/logs"));
}

#[cfg(windows)]
#[test]
fn portable_roots_turn_every_windows_root_into_forward_slashes() {
    let mut source = fixture().source;
    source.home_dir = r"C:\pm3".to_string();
    source.apps_dir = r"C:\pm3\apps".to_string();
    source.state_dir = r"C:\pm3\state".to_string();
    source.runtime_dir = r"C:\pm3\run".to_string();
    source.data_dir = r"C:\pm3\data".to_string();
    source.logs_dir = r"C:\pm3\logs".to_string();
    source.cfg_dir = PathBuf::from(r"C:\pm3\service");
    source.host_home = Some(r"C:\Users\dev".to_string());
    source.tmp_dir = Some(r"C:\Temp".to_string());
    let source = source.with_portable_roots();
    assert_eq!(
        [
            source.home_dir.as_str(),
            &source.apps_dir,
            &source.state_dir,
            &source.runtime_dir,
            &source.data_dir,
            &source.logs_dir,
        ],
        [
            "C:/pm3",
            "C:/pm3/apps",
            "C:/pm3/state",
            "C:/pm3/run",
            "C:/pm3/data",
            "C:/pm3/logs"
        ]
    );
    assert_eq!(source.cfg_dir, PathBuf::from("C:/pm3/service"));
    assert_eq!(source.host_home.as_deref(), Some("C:/Users/dev"));
    assert_eq!(source.tmp_dir.as_deref(), Some("C:/Temp"));
}

#[test]
fn a_service_file_is_named_after_the_service() {
    let path = service_file_of(Path::new("/etc/pm3"), "web");
    assert_eq!(path, Ok(PathBuf::from("/etc/pm3/web.yaml")));
}

#[test]
fn a_dotted_service_name_keeps_every_part() {
    let path = service_file_of(Path::new("/etc/pm3"), "api.v2");
    assert_eq!(path, Ok(PathBuf::from("/etc/pm3/api.v2.yaml")));
}

#[test]
fn the_source_locates_the_service_file_in_the_config_directory() {
    let fixture = fixture();
    let expected = fixture.dir.path().join("service/web.yaml");
    assert_eq!(fixture.source.service("web"), Ok(expected));
}

#[test]
fn the_defaults_come_from_the_configured_restart_policy() {
    let fixture = fixture();
    let defaults = fixture.source.defaults().expect("defaults should build");
    assert_eq!(defaults.restart.max_restarts, 15);
}

#[test]
fn an_unusable_sandbox_mode_stops_the_defaults() {
    let mut fixture = fixture();
    fixture.source.config.sandbox.mode = "yolo".to_string();
    let err = fixture.source.defaults().unwrap_err().to_string();
    assert!(
        err.contains("cannot accept sandbox mode 'yolo'"),
        "got: {err}"
    );
}

#[tokio::test]
async fn resolving_a_service_reads_its_own_file() {
    let fixture = fixture();
    register_service(&fixture.source, "web");
    let spec = fixture
        .source
        .resolve_service("web")
        .await
        .expect("the service should resolve");
    assert_eq!(spec.script, SERVICE_SCRIPT);
}

#[tokio::test]
async fn resolving_a_service_defaults_the_working_directory_to_the_pm3_home() {
    let fixture = fixture();
    register_service(&fixture.source, "web");
    let spec = fixture
        .source
        .resolve_service("web")
        .await
        .expect("the service should resolve");
    let expected = crate::portable_real_path(
        &std::fs::canonicalize(fixture.dir.path().join("web")).expect("canonicalize the workspace"),
    );
    assert_eq!(spec.cwd, expected);
}

#[tokio::test]
async fn resolving_a_service_expands_the_home_placeholder() {
    let fixture = fixture();
    write_service_file(
        &fixture.source,
        "web",
        &format!("name: \"web\"\nscript: '{SERVICE_SCRIPT}'\nargs:\n  - \"${{HOME}}/app.js\"\n"),
    );
    let spec = fixture
        .source
        .resolve_service("web")
        .await
        .expect("the service should resolve");
    assert!(!spec.args[0].contains("${HOME}"), "got: {:?}", spec.args);
}

#[tokio::test]
async fn resolving_a_service_without_a_file_is_reported() {
    let fixture = fixture();
    let err = fixture
        .source
        .resolve_service("web")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot read apps file"), "got: {err}");
}

#[tokio::test]
async fn resolving_a_service_that_its_file_does_not_declare_is_reported() {
    let fixture = fixture();
    write_service_file(&fixture.source, "web", &service_yaml("db"));
    let err = fixture
        .source
        .resolve_service("web")
        .await
        .unwrap_err()
        .to_string();
    assert_eq!(err, "cannot find app 'web' in its own service file");
}

#[tokio::test]
async fn resolving_a_service_from_an_empty_file_is_reported() {
    let fixture = fixture();
    write_service_file(&fixture.source, "web", "\n");
    let err = fixture
        .source
        .resolve_service("web")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.starts_with("cannot parse apps file"), "got: {err}");
}

#[tokio::test]
async fn resolving_a_service_with_an_unusable_sandbox_mode_is_reported() {
    let mut fixture = fixture();
    register_service(&fixture.source, "web");
    fixture.source.config.sandbox.mode = "yolo".to_string();
    let err = fixture
        .source
        .resolve_service("web")
        .await
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("cannot accept sandbox mode 'yolo'"),
        "got: {err}"
    );
}

#[tokio::test]
async fn resolving_a_service_from_a_legacy_apps_file_is_reported() {
    let fixture = fixture();
    write_service_file(
        &fixture.source,
        "web",
        &format!("apps:\n  - name: \"web\"\n    script: '{SERVICE_SCRIPT}'\n"),
    );
    let err = fixture
        .source
        .resolve_service("web")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.starts_with("cannot parse apps file"), "got: {err}");
}

#[tokio::test]
async fn resolving_a_service_without_an_environment_file_still_hands_out_the_home() {
    let fixture = fixture();
    register_service(&fixture.source, "web");
    let spec = fixture
        .source
        .resolve_service("web")
        .await
        .expect("a missing environment file is fine");
    assert_eq!(
        spec.env,
        [EnvValue::injected("HOME", HOST_HOME)],
        "a service must not have to spell out an absolute home"
    );
}

#[tokio::test]
async fn a_declared_home_wins_over_the_one_pm3_hands_out() {
    let fixture = fixture();
    register_service(&fixture.source, "web");
    write_env_file(&fixture.source, "web", "HOME=/srv/web\n");
    let spec = fixture
        .source
        .resolve_service("web")
        .await
        .expect("the service should resolve");
    assert_eq!(
        spec.env,
        [EnvValue::new("HOME", "/srv/web", EnvScope::App)],
        "got: {:?}",
        spec.env
    );
}

#[tokio::test]
async fn a_host_without_a_home_hands_out_nothing() {
    let mut fixture = fixture();
    fixture.source.host_home = None;
    register_service(&fixture.source, "web");
    let spec = fixture
        .source
        .resolve_service("web")
        .await
        .expect("the service should resolve");
    assert_eq!(spec.env, []);
}

#[tokio::test]
async fn resolving_a_service_loads_the_environment_beside_its_file() {
    let fixture = fixture();
    register_service(&fixture.source, "web");
    write_env_file(
        &fixture.source,
        "web",
        "# the tunnel credential\nTUNNEL_TOKEN=eyJhIjoiZjQ2\nPORT=8080\n",
    );
    let spec = fixture
        .source
        .resolve_service("web")
        .await
        .expect("the service should resolve");
    assert_eq!(
        spec.env,
        [
            EnvValue::injected("HOME", HOST_HOME),
            EnvValue::new("PORT", "8080", EnvScope::App),
            EnvValue::new("TUNNEL_TOKEN", "eyJhIjoiZjQ2", EnvScope::App),
        ]
    );
}

#[tokio::test]
async fn resolving_a_service_with_an_unparsable_environment_file_is_reported() {
    let fixture = fixture();
    register_service(&fixture.source, "web");
    write_env_file(&fixture.source, "web", "TUNNEL_TOKEN\n");
    let err = fixture
        .source
        .resolve_service("web")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("expected KEY=VALUE"), "got: {err}");
}

#[tokio::test]
async fn resolving_a_service_whose_file_declares_an_environment_is_refused() {
    let fixture = fixture();
    write_service_file(
        &fixture.source,
        "web",
        &format!(
            "name: \"web\"\nscript: '{SERVICE_SCRIPT}'\nenv:\n  TUNNEL_TOKEN: \"eyJhIjoiZjQ2\"\n"
        ),
    );
    let err = fixture
        .source
        .resolve_service("web")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("'web.env'"), "got: {err}");
    assert!(!err.contains("eyJhIjoiZjQ2"), "got: {err}");
}

#[tokio::test]
async fn resolving_a_service_whose_name_escapes_the_config_directory_is_refused() {
    let fixture = fixture();
    let err = fixture
        .source
        .resolve_service("../escape")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot accept app name"), "got: {err}");
}

#[tokio::test]
async fn a_writable_root_linking_into_a_hidden_root_fails_the_prepare() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let canonical = PathBuf::from(crate::platform::real_text(dir.path()));
    let source = spec_source_in(&canonical);
    let link = canonical.join("data");
    crate::platform::link_dir(&canonical, &link);
    write_service_file(
        &source,
        "web",
        &format!(
            "name: \"web\"\nscript: '{SERVICE_SCRIPT}'\nsandbox:\n  writable_roots:\n    - \"{}\"\n",
            crate::platform::text(&link)
        ),
    );
    let err = source.prepare("web").await.unwrap_err().to_string();
    assert!(err.contains("keeps out of every sandbox"), "got: {err}");
}

#[tokio::test]
async fn a_plain_environment_is_remembered_as_its_own_origin() {
    let fixture = fixture();
    register_service(&fixture.source, "web");
    write_env_file(&fixture.source, "web", "SOURCE=plaintext\n");
    let spec = fixture
        .source
        .resolve_service("web")
        .await
        .expect("the service should resolve");
    assert_eq!(spec.env_origin, usecases::EnvOrigin::Plain);
}

#[tokio::test]
async fn the_spec_counts_only_the_values_the_operator_declared() {
    let fixture = fixture();
    register_service(&fixture.source, "web");
    write_env_file(&fixture.source, "web", "A=1\nB=2\n");
    let spec = fixture
        .source
        .resolve_service("web")
        .await
        .expect("the service should resolve");
    assert_eq!(
        spec.declared_env_count(),
        2,
        "the HOME pm3 injects is not something the operator declared, got {:?}",
        spec.env
    );
    assert_eq!(spec.env.len(), 3);
}

#[tokio::test]
async fn a_shared_value_reaches_an_app_that_declares_nothing() {
    let mut fixture = fixture();
    with_global_env(&mut fixture.source, &[("TZ", "UTC")]);
    register_service(&fixture.source, "web");
    let spec = fixture
        .source
        .resolve_service("web")
        .await
        .expect("the service should resolve");
    assert!(
        spec.env
            .contains(&EnvValue::new("TZ", "UTC", EnvScope::Global)),
        "got: {:?}",
        spec.env
    );
}

#[tokio::test]
async fn an_app_value_wins_over_a_shared_one_with_the_same_key() {
    let mut fixture = fixture();
    with_global_env(&mut fixture.source, &[("TZ", "UTC")]);
    register_service(&fixture.source, "web");
    write_env_file(&fixture.source, "web", "TZ=Asia/Shanghai\n");
    let spec = fixture
        .source
        .resolve_service("web")
        .await
        .expect("the service should resolve");
    assert!(
        spec.env
            .contains(&EnvValue::new("TZ", "Asia/Shanghai", EnvScope::App)),
        "the app that spells a value out owns it, got: {:?}",
        spec.env
    );
}

#[tokio::test]
async fn a_shared_value_wins_over_the_home_pm3_injects() {
    let mut fixture = fixture();
    with_global_env(&mut fixture.source, &[("HOME", "/srv/shared")]);
    register_service(&fixture.source, "web");
    let spec = fixture
        .source
        .resolve_service("web")
        .await
        .expect("the service should resolve");
    assert_eq!(
        spec.env,
        [EnvValue::new("HOME", "/srv/shared", EnvScope::Global)],
        "got: {:?}",
        spec.env
    );
}

#[tokio::test]
async fn the_declared_count_covers_the_shared_values_too() {
    let mut fixture = fixture();
    with_global_env(&mut fixture.source, &[("TZ", "UTC")]);
    register_service(&fixture.source, "web");
    write_env_file(&fixture.source, "web", "PORT=8080\n");
    let spec = fixture
        .source
        .resolve_service("web")
        .await
        .expect("the service should resolve");
    assert_eq!(
        spec.declared_env_count(),
        2,
        "a shared value is declared by the operator too, got: {:?}",
        spec.env
    );
}

#[path = "apps_file_source_secrets_tests.rs"]
mod secrets;
