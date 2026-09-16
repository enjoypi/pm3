use super::*;
use crate::config_sections::{pm3_section, telemetry_section};

fn config() -> Pm3Config {
    let yaml = format!(
        "{}{}",
        pm3_section("/tmp/pm3-fixture", 1600, "workspace-write"),
        telemetry_section("info"),
    );
    crate::config::parse_config(&yaml)
        .expect("the fixture config should parse")
        .pm3
}

fn write(root: &Path, suffix: &str, body: &str) {
    std::fs::write(root.join(format!("{GLOBAL_ENV_STEM}.{suffix}")), body)
        .expect("write the shared environment file");
}

#[tokio::test]
async fn a_missing_shared_environment_hands_out_nothing() {
    let dir = tempfile::tempdir().expect("temp dir");
    let values = load_global_env(&config(), dir.path(), None)
        .await
        .expect("a missing shared environment is fine");
    assert!(values.is_empty(), "got: {values:?}");
}

#[tokio::test]
async fn every_shared_value_carries_the_global_scope() {
    let dir = tempfile::tempdir().expect("temp dir");
    write(
        dir.path(),
        ENV_FILE_SUFFIX,
        "TZ=UTC\nXDG_DATA_HOME=/srv/data\n",
    );
    let values = load_global_env(&config(), dir.path(), None)
        .await
        .expect("the shared environment should load");
    assert_eq!(
        values,
        vec![
            EnvValue::global("TZ", "UTC"),
            EnvValue::global("XDG_DATA_HOME", "/srv/data"),
        ],
        "got: {values:?}"
    );
}

#[tokio::test]
async fn a_shared_value_expands_the_home_placeholder() {
    let dir = tempfile::tempdir().expect("temp dir");
    write(
        dir.path(),
        ENV_FILE_SUFFIX,
        "XDG_DATA_HOME=$HOME/.local/share\n",
    );
    let values = load_global_env(&config(), dir.path(), Some("/home/dev"))
        .await
        .expect("the shared environment should load");
    assert_eq!(
        values,
        vec![EnvValue::global("XDG_DATA_HOME", "/home/dev/.local/share")],
        "got: {values:?}"
    );
}

#[tokio::test]
async fn an_unparsable_shared_environment_is_reported() {
    let dir = tempfile::tempdir().expect("temp dir");
    write(dir.path(), ENV_FILE_SUFFIX, "TZ\n");
    let err = load_global_env(&config(), dir.path(), None)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("expected KEY=VALUE"), "got: {err}");
}

#[cfg(unix)]
#[tokio::test]
async fn a_sealed_shared_environment_falls_back_to_the_plain_one() {
    let dir = tempfile::tempdir().expect("temp dir");
    write(dir.path(), ENC_FILE_SUFFIX, "sops: {}\n");
    write(dir.path(), ENV_FILE_SUFFIX, "TZ=UTC\n");
    let values = load_global_env(&config(), dir.path(), None)
        .await
        .expect("a sealed sidecar must not stop pm3");
    assert_eq!(
        values,
        vec![EnvValue::global("TZ", "UTC")],
        "got: {values:?}"
    );
}

#[cfg(unix)]
fn with_decryptor(config: &mut Pm3Config, dir: &Path, body: &str) {
    use std::os::unix::fs::PermissionsExt as _;

    let path = dir.join("fake-sops");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write the decryptor stub");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
        .expect("make the decryptor stub executable");
    config.sops_program = path.to_string_lossy().into_owned();
    config.sops_identity_file = "/home/dev/.ssh/age-shared".to_string();
}

#[cfg(unix)]
#[tokio::test]
async fn an_opened_shared_environment_reaches_every_app() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut config = config();
    with_decryptor(&mut config, dir.path(), "printf 'TZ=UTC\\n'");
    write(dir.path(), ENC_FILE_SUFFIX, "sops: {}\n");

    let values = load_global_env(&config, dir.path(), None)
        .await
        .expect("the decryptor should hand over the values");

    assert_eq!(
        values,
        vec![EnvValue::global("TZ", "UTC")],
        "got: {values:?}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn an_opened_shared_value_keeps_its_quotes_verbatim() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut config = config();
    with_decryptor(&mut config, dir.path(), "printf 'PASSWORD=\"p@ss\\n'");
    write(dir.path(), ENC_FILE_SUFFIX, "sops: {}\n");

    let values = load_global_env(&config, dir.path(), None)
        .await
        .expect("the decryptor should hand over the values");

    assert_eq!(
        values,
        vec![EnvValue::global("PASSWORD", "\"p@ss")],
        "sops already parsed the value, got: {values:?}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_decryptor_that_refuses_stops_the_shared_environment() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut config = config();
    with_decryptor(&mut config, dir.path(), "exit 1");
    write(dir.path(), ENC_FILE_SUFFIX, "sops: {}\n");

    let err = load_global_env(&config, dir.path(), None)
        .await
        .unwrap_err()
        .to_string();

    assert!(err.contains("cannot decrypt"), "got: {err}");
}

#[cfg(unix)]
#[tokio::test]
async fn an_opened_shared_value_expands_the_home_placeholder() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut config = config();
    with_decryptor(&mut config, dir.path(), "printf 'BIN=$HOME/bin\\n'");
    write(dir.path(), ENC_FILE_SUFFIX, "sops: {}\n");

    let values = load_global_env(&config, dir.path(), Some("/home/dev"))
        .await
        .expect("the decryptor should hand over the values");

    assert_eq!(
        values,
        vec![EnvValue::global("BIN", "/home/dev/bin")],
        "got: {values:?}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn an_opened_shared_environment_that_will_not_parse_is_reported() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut config = config();
    with_decryptor(&mut config, dir.path(), "printf 'TZ\\n'");
    write(dir.path(), ENC_FILE_SUFFIX, "sops: {}\n");

    let err = load_global_env(&config, dir.path(), None)
        .await
        .unwrap_err()
        .to_string();

    assert!(err.contains("expected KEY=VALUE"), "got: {err}");
}

#[tokio::test]
async fn a_shared_sidecar_that_cannot_be_stat_ed_is_reported() {
    let dir = tempfile::tempdir().expect("temp dir");
    let blocked = dir.path().join("blocked");
    std::fs::write(&blocked, "not a directory").expect("occupy the config root");

    let err = load_global_env(&config(), &blocked, None)
        .await
        .unwrap_err()
        .to_string();

    assert!(
        err.contains("cannot reach") || err.contains("Not a directory"),
        "got: {err}"
    );
}
