use super::*;
use crate::spec_sources::{
    SERVICE_SCRIPT, register_service, service_yaml, spec_source_in, write_service_file,
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
fn a_service_file_is_named_after_the_service() {
    let path = service_file_of(Path::new("/etc/pm3"), "web");
    assert_eq!(path, PathBuf::from("/etc/pm3/web.yaml"));
}

#[test]
fn a_dotted_service_name_keeps_every_part() {
    let path = service_file_of(Path::new("/etc/pm3"), "api.v2");
    assert_eq!(path, PathBuf::from("/etc/pm3/api.v2.yaml"));
}

#[test]
fn the_source_locates_the_service_file_in_the_config_directory() {
    let fixture = fixture();
    let expected = fixture.dir.path().join("svc/web.yaml");
    assert_eq!(fixture.source.service_file("web"), expected);
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

#[test]
fn resolving_a_service_reads_its_own_file() {
    let fixture = fixture();
    register_service(&fixture.source, "web");
    let spec = fixture
        .source
        .resolve_service("web")
        .expect("the service should resolve");
    assert_eq!(spec.script, SERVICE_SCRIPT);
}

#[test]
fn resolving_a_service_defaults_the_working_directory_to_the_pm3_home() {
    let fixture = fixture();
    register_service(&fixture.source, "web");
    let spec = fixture
        .source
        .resolve_service("web")
        .expect("the service should resolve");
    let expected = fixture
        .dir
        .path()
        .join("web")
        .to_string_lossy()
        .into_owned();
    assert_eq!(spec.cwd, expected);
}

#[test]
fn resolving_a_service_expands_the_home_placeholder() {
    let fixture = fixture();
    write_service_file(
        &fixture.source,
        "web",
        "apps:\n  - name: \"web\"\n    script: \"/bin/sh\"\n    args:\n      - \"${HOME}/app.js\"\n",
    );
    let spec = fixture
        .source
        .resolve_service("web")
        .expect("the service should resolve");
    assert!(!spec.args[0].contains("${HOME}"), "got: {:?}", spec.args);
}

#[test]
fn resolving_a_service_without_a_file_is_reported() {
    let fixture = fixture();
    let err = fixture
        .source
        .resolve_service("web")
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot read apps file"), "got: {err}");
}

#[test]
fn resolving_a_service_that_its_file_does_not_declare_is_reported() {
    let fixture = fixture();
    write_service_file(&fixture.source, "web", &service_yaml("db"));
    let err = fixture
        .source
        .resolve_service("web")
        .unwrap_err()
        .to_string();
    assert_eq!(err, "cannot find app 'web' in its own service file");
}

#[test]
fn resolving_a_service_from_an_empty_file_is_reported() {
    let fixture = fixture();
    write_service_file(&fixture.source, "web", "apps: []\n");
    let err = fixture
        .source
        .resolve_service("web")
        .unwrap_err()
        .to_string();
    assert_eq!(err, "cannot accept an apps file with no apps");
}

#[test]
fn resolving_a_service_with_an_unusable_sandbox_mode_is_reported() {
    let mut fixture = fixture();
    register_service(&fixture.source, "web");
    fixture.source.config.sandbox.mode = "yolo".to_string();
    let err = fixture
        .source
        .resolve_service("web")
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("cannot accept sandbox mode 'yolo'"),
        "got: {err}"
    );
}
