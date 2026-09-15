use super::*;
use crate::spec_sources::{
    HOST_HOME, SERVICE_SCRIPT, register_service, service_yaml, spec_source_in, write_env_file,
    write_service_file,
};
#[cfg(unix)]
use crate::spec_sources::{with_decryptor, write_enc_file};

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
    let expected = std::fs::canonicalize(fixture.dir.path().join("web"))
        .expect("canonicalize the workspace")
        .to_string_lossy()
        .into_owned();
    assert_eq!(spec.cwd, expected);
}

#[tokio::test]
async fn resolving_a_service_expands_the_home_placeholder() {
    let fixture = fixture();
    write_service_file(
        &fixture.source,
        "web",
        "name: \"web\"\nscript: \"/bin/sh\"\nargs:\n  - \"${HOME}/app.js\"\n",
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
        "apps:\n  - name: \"web\"\n    script: \"/bin/sh\"\n",
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
        [("HOME".to_string(), HOST_HOME.to_string())],
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
    assert_eq!(spec.env, [("HOME".to_string(), "/srv/web".to_string())]);
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
    assert!(spec.env.is_empty());
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
            ("HOME".to_string(), HOST_HOME.to_string()),
            ("PORT".to_string(), "8080".to_string()),
            ("TUNNEL_TOKEN".to_string(), "eyJhIjoiZjQ2".to_string()),
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
        "name: \"web\"\nscript: \"/bin/sh\"\nenv:\n  TUNNEL_TOKEN: \"eyJhIjoiZjQ2\"\n",
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

#[test]
fn a_service_name_that_escapes_the_config_directory_has_no_path() {
    let path = service_file_of(Path::new("/etc/pm3"), "../escape");
    assert!(path.is_err(), "got: {path:?}");
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

#[cfg(unix)]
#[tokio::test]
async fn a_writable_root_linking_into_a_hidden_root_fails_the_prepare() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let canonical = dir.path().canonicalize().expect("canonical temp dir");
    let source = spec_source_in(&canonical);
    let link = canonical.join("data");
    std::os::unix::fs::symlink(&canonical, &link).expect("link into the pm3 home");
    write_service_file(
        &source,
        "web",
        &format!(
            "name: \"web\"\nscript: \"/bin/sh\"\nsandbox:\n  writable_roots:\n    - \"{}\"\n",
            link.display()
        ),
    );
    let err = source.prepare("web").await.unwrap_err().to_string();
    assert!(err.contains("keeps out of every sandbox"), "got: {err}");
}

#[cfg(unix)]
#[tokio::test]
async fn an_encrypted_environment_reaches_the_service() {
    let mut fixture = fixture();
    register_service(&fixture.source, "web");
    write_enc_file(&fixture.source, "web", "TUNNEL_TOKEN: ENC[fake]\n");
    with_decryptor(&mut fixture.source, "printf 'TUNNEL_TOKEN=eyJhIjoiZjQ2\\n'");
    let spec = fixture
        .source
        .resolve_service("web")
        .await
        .expect("the service should resolve");
    assert_eq!(
        spec.env,
        [
            ("HOME".to_string(), HOST_HOME.to_string()),
            ("TUNNEL_TOKEN".to_string(), "eyJhIjoiZjQ2".to_string()),
        ]
    );
}

#[cfg(unix)]
#[tokio::test]
async fn an_encrypted_environment_wins_over_a_plaintext_one() {
    let mut fixture = fixture();
    register_service(&fixture.source, "web");
    write_env_file(&fixture.source, "web", "SOURCE=plaintext\n");
    write_enc_file(&fixture.source, "web", "SOURCE: ENC[fake]\n");
    with_decryptor(&mut fixture.source, "printf 'SOURCE=encrypted\\n'");
    let spec = fixture
        .source
        .resolve_service("web")
        .await
        .expect("the service should resolve");
    assert!(
        spec.env
            .contains(&("SOURCE".to_string(), "encrypted".to_string())),
        "the encrypted sidecar is the one that counts, got {:?}",
        spec.env
    );
}

#[cfg(unix)]
#[tokio::test]
async fn an_encrypted_environment_is_ignored_without_an_identity() {
    let mut fixture = fixture();
    register_service(&fixture.source, "web");
    write_env_file(&fixture.source, "web", "SOURCE=plaintext\n");
    write_enc_file(&fixture.source, "web", "SOURCE: ENC[fake]\n");
    with_decryptor(&mut fixture.source, "printf 'SOURCE=encrypted\\n'");
    fixture.source.config.sops_identity_file = String::new();
    let spec = fixture
        .source
        .resolve_service("web")
        .await
        .expect("the service should resolve");
    assert!(
        spec.env
            .contains(&("SOURCE".to_string(), "plaintext".to_string())),
        "an unconfigured identity leaves the encrypted sidecar untouched, got {:?}",
        spec.env
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_failing_decryptor_stops_the_service_from_resolving() {
    let mut fixture = fixture();
    register_service(&fixture.source, "web");
    write_enc_file(&fixture.source, "web", "TUNNEL_TOKEN: ENC[fake]\n");
    with_decryptor(&mut fixture.source, "exit 4");
    let err = fixture
        .source
        .resolve_service("web")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("exited with status 4"), "got: {err}");
}

#[cfg(unix)]
#[tokio::test]
async fn an_encrypted_value_may_hold_spaces() {
    let mut fixture = fixture();
    register_service(&fixture.source, "web");
    write_enc_file(&fixture.source, "web", "MOTTO: ENC[fake]\n");
    with_decryptor(&mut fixture.source, "printf 'MOTTO=two words\\n'");
    let spec = fixture
        .source
        .resolve_service("web")
        .await
        .expect("the service should resolve");
    assert!(
        spec.env
            .contains(&("MOTTO".to_string(), "two words".to_string())),
        "an unquoted dotenv value keeps its spaces, got {:?}",
        spec.env
    );
}

#[cfg(unix)]
#[tokio::test]
async fn an_encrypted_environment_expands_the_host_home() {
    let mut fixture = fixture();
    register_service(&fixture.source, "web");
    write_enc_file(&fixture.source, "web", "BIN: ENC[fake]\n");
    with_decryptor(&mut fixture.source, "printf 'BIN=$HOME/bin\\n'");
    let spec = fixture
        .source
        .resolve_service("web")
        .await
        .expect("the service should resolve");
    assert!(
        spec.env
            .contains(&("BIN".to_string(), format!("{HOST_HOME}/bin"))),
        "the decrypted path goes through the same $HOME expansion, got {:?}",
        spec.env
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_service_without_an_encrypted_sidecar_still_reads_its_plaintext_one() {
    let mut fixture = fixture();
    register_service(&fixture.source, "web");
    write_env_file(&fixture.source, "web", "SOURCE=plaintext\n");
    with_decryptor(&mut fixture.source, "printf 'SOURCE=encrypted\\n'");
    let spec = fixture
        .source
        .resolve_service("web")
        .await
        .expect("the service should resolve");
    assert!(
        spec.env
            .contains(&("SOURCE".to_string(), "plaintext".to_string())),
        "a configured identity alone must not conjure an encrypted sidecar, got {:?}",
        spec.env
    );
}

#[cfg(unix)]
#[tokio::test]
async fn an_unparsable_decrypted_environment_is_reported() {
    let mut fixture = fixture();
    register_service(&fixture.source, "web");
    write_enc_file(&fixture.source, "web", "TUNNEL_TOKEN: ENC[fake]\n");
    with_decryptor(&mut fixture.source, "printf 'TUNNEL_TOKEN\\n'");
    let err = fixture
        .source
        .resolve_service("web")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("expected KEY=VALUE"), "got: {err}");
}

#[cfg(unix)]
#[tokio::test]
async fn an_encrypted_value_reaches_the_service_exactly_as_it_was_decrypted() {
    let mut fixture = fixture();
    register_service(&fixture.source, "web");
    write_enc_file(&fixture.source, "web", "PASSWORD: ENC[fake]\n");
    with_decryptor(&mut fixture.source, "printf 'PASSWORD=\"p@ss\\\\word\"\\n'");
    let spec = fixture
        .source
        .resolve_service("web")
        .await
        .expect("the service should resolve");
    assert!(
        spec.env
            .contains(&("PASSWORD".to_string(), "\"p@ss\\word\"".to_string())),
        "a decrypted secret is handed over untouched, got {:?}",
        spec.env
    );
}

#[cfg(unix)]
#[tokio::test]
async fn an_unreachable_encrypted_sidecar_stops_the_service_from_resolving() {
    let mut fixture = fixture();
    register_service(&fixture.source, "web");
    let path = crate::enc_file_of(&fixture.source.cfg_dir, "web").expect("a safe service name");
    std::os::unix::fs::symlink(&path, &path).expect("seed a looping symlink");
    with_decryptor(&mut fixture.source, "printf 'A=b\\n'");
    let err = fixture
        .source
        .resolve_service("web")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot look at"), "got: {err}");
}

#[cfg(unix)]
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

#[cfg(unix)]
#[tokio::test]
async fn a_decrypted_environment_is_remembered_as_its_own_origin() {
    let mut fixture = fixture();
    register_service(&fixture.source, "web");
    write_enc_file(&fixture.source, "web", "TOKEN: ENC[fake]\n");
    with_decryptor(&mut fixture.source, "printf 'TOKEN=abc\\n'");
    let spec = fixture
        .source
        .resolve_service("web")
        .await
        .expect("the service should resolve");
    assert_eq!(spec.env_origin, usecases::EnvOrigin::Encrypted);
}

#[cfg(unix)]
#[tokio::test]
async fn a_sidecar_pm3_never_opened_is_remembered_as_sealed() {
    let mut fixture = fixture();
    register_service(&fixture.source, "web");
    write_env_file(&fixture.source, "web", "SOURCE=plaintext\n");
    write_enc_file(&fixture.source, "web", "TOKEN: ENC[fake]\n");
    with_decryptor(&mut fixture.source, "printf 'TOKEN=abc\\n'");
    fixture.source.config.sops_identity_file = String::new();
    let spec = fixture
        .source
        .resolve_service("web")
        .await
        .expect("the service should resolve");
    assert_eq!(
        spec.env_origin,
        usecases::EnvOrigin::Sealed,
        "an app falling back to plaintext because no identity is configured must still stand out"
    );
    assert!(
        spec.env
            .contains(&("SOURCE".to_string(), "plaintext".to_string())),
        "got {:?}",
        spec.env
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_sealed_sidecar_still_reports_a_broken_plaintext_fallback() {
    let mut fixture = fixture();
    register_service(&fixture.source, "web");
    write_env_file(&fixture.source, "web", "BROKEN\n");
    write_enc_file(&fixture.source, "web", "TOKEN: ENC[fake]\n");
    with_decryptor(&mut fixture.source, "printf 'TOKEN=abc\\n'");
    fixture.source.config.sops_identity_file = String::new();
    let err = fixture
        .source
        .resolve_service("web")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("expected KEY=VALUE"), "got: {err}");
}

#[cfg(unix)]
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
        spec.env_declared, 2,
        "the HOME pm3 injects is not something the operator declared, got {:?}",
        spec.env
    );
    assert_eq!(spec.env.len(), 3);
}
