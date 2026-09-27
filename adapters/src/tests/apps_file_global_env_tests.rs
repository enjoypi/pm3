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
    assert_eq!(values, [], "got: {values:?}");
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

#[tokio::test]
async fn an_undecryptable_sidecar_without_an_identity_falls_back_to_the_plain_one() {
    let dir = tempfile::tempdir().expect("temp dir");
    write(dir.path(), ENC_FILE_SUFFIX, "sops: {}\n");
    write(dir.path(), ENV_FILE_SUFFIX, "TZ=UTC\n");
    let values = load_global_env(&config(), dir.path(), None)
        .await
        .expect("without a declared identity pm3 only tries, it does not insist");
    assert_eq!(
        values,
        vec![EnvValue::global("TZ", "UTC")],
        "got: {values:?}"
    );
}

fn with_decryptor(config: &mut Pm3Config, dir: &Path, unix: &str, windows: &str) {
    let path = crate::platform::script(dir, "fake-sops", unix, windows);
    config.sops_program = path.to_string_lossy().into_owned();
    config.sops_identity_file = "/home/dev/.ssh/age-shared".to_string();
}

#[tokio::test]
async fn an_opened_shared_environment_reaches_every_app() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut config = config();
    with_decryptor(&mut config, dir.path(), "printf 'TZ=UTC\\n'", "echo TZ=UTC");
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

#[tokio::test]
async fn an_opened_shared_value_keeps_its_quotes_verbatim() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut config = config();
    with_decryptor(
        &mut config,
        dir.path(),
        "printf 'PASSWORD=\"p@ss\\n'",
        "echo PASSWORD=\"p@ss",
    );
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

#[tokio::test]
async fn a_decryptor_that_refuses_stops_the_shared_environment() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut config = config();
    with_decryptor(&mut config, dir.path(), "exit 1", "exit /b 1");
    write(dir.path(), ENC_FILE_SUFFIX, "sops: {}\n");

    let err = load_global_env(&config, dir.path(), None)
        .await
        .unwrap_err()
        .to_string();

    assert!(err.contains("cannot decrypt"), "got: {err}");
}

#[tokio::test]
async fn an_opened_shared_value_expands_the_home_placeholder() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut config = config();
    with_decryptor(
        &mut config,
        dir.path(),
        "printf 'BIN=$HOME/bin\\n'",
        "echo BIN=$HOME/bin",
    );
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

#[tokio::test]
async fn an_opened_shared_environment_that_will_not_parse_is_reported() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut config = config();
    with_decryptor(&mut config, dir.path(), "printf 'TZ\\n'", "echo TZ");
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
    let blocked = crate::platform::unreachable_parent(dir.path());

    let err = load_global_env(&config(), &blocked, None)
        .await
        .unwrap_err()
        .to_string();

    assert!(
        err.starts_with("cannot read the environment file"),
        "got: {err}"
    );
}

#[tokio::test]
async fn the_plain_layer_hands_its_xdg_values_to_the_decryptor() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut config = config();
    with_decryptor(
        &mut config,
        dir.path(),
        "printf 'SEEN=%s\\n' \"$XDG_CONFIG_HOME\"",
        "echo SEEN=%XDG_CONFIG_HOME%",
    );
    write(dir.path(), ENC_FILE_SUFFIX, "sops: {}\n");
    write(
        dir.path(),
        ENV_FILE_SUFFIX,
        "XDG_CONFIG_HOME=/srv/config\nTZ=UTC\n",
    );

    let values = load_global_env(&config, dir.path(), None)
        .await
        .expect("the shared environment should load");

    assert!(
        values.contains(&EnvValue::global("SEEN", "/srv/config")),
        "the decryptor must see the plain layer's xdg values, got: {values:?}"
    );
}

#[tokio::test]
async fn an_opened_shared_value_wins_over_the_plain_one() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut config = config();
    with_decryptor(
        &mut config,
        dir.path(),
        "printf 'TZ=Asia/Shanghai\\n'",
        "echo TZ=Asia/Shanghai",
    );
    write(dir.path(), ENC_FILE_SUFFIX, "sops: {}\n");
    write(dir.path(), ENV_FILE_SUFFIX, "TZ=UTC\nLANG=C\n");

    let values = load_global_env(&config, dir.path(), None)
        .await
        .expect("the shared environment should load");

    assert!(
        values.contains(&EnvValue::global("TZ", "Asia/Shanghai")),
        "the encrypted layer owns a key it declares, got: {values:?}"
    );
    assert!(
        values.contains(&EnvValue::global("LANG", "C")),
        "a plain-only key survives, got: {values:?}"
    );
}

#[tokio::test]
async fn a_sidecar_opens_with_the_xdg_values_from_the_plain_layer() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut config = config();
    with_decryptor(
        &mut config,
        dir.path(),
        "printf 'TOKEN=%s\\n' \"$XDG_CONFIG_HOME\"",
        "echo TOKEN=%XDG_CONFIG_HOME%",
    );
    config.sops_identity_file = String::new();
    write(dir.path(), ENC_FILE_SUFFIX, "sops: {}\n");
    write(dir.path(), ENV_FILE_SUFFIX, "XDG_CONFIG_HOME=/srv/cfg\n");

    let values = load_global_env(&config, dir.path(), None)
        .await
        .expect("a decryptor that finds its own key needs no declared identity");

    assert!(
        values.contains(&EnvValue::global("XDG_CONFIG_HOME", "/srv/cfg")),
        "the plain layer still stands on its own, got: {values:?}"
    );
    assert!(
        values.contains(&EnvValue::global("TOKEN", "/srv/cfg")),
        "the decryptor saw the plain layer's xdg values even without an identity, got: {values:?}"
    );
}

#[test]
fn the_decryptor_environment_keeps_only_the_xdg_values() {
    let global = vec![
        EnvValue::global("XDG_CONFIG_HOME", "/srv/cfg"),
        EnvValue::global("TZ", "UTC"),
        EnvValue::global("XDG_DATA_HOME", "/srv/data"),
    ];
    let kept = decryptor_env(&global);
    assert_eq!(
        kept,
        vec![
            ("XDG_CONFIG_HOME".to_string(), "/srv/cfg".to_string()),
            ("XDG_DATA_HOME".to_string(), "/srv/data".to_string()),
        ],
        "only the values that tell a program where to look travel to the decryptor, got: {kept:?}"
    );
}

#[tokio::test]
async fn a_decryptor_that_needs_no_identity_still_hands_over_its_values() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut config = config();
    with_decryptor(&mut config, dir.path(), "printf 'TZ=UTC\\n'", "echo TZ=UTC");
    config.sops_identity_file = String::new();
    write(dir.path(), ENC_FILE_SUFFIX, "sops: {}\n");

    let values = load_global_env(&config, dir.path(), None)
        .await
        .expect("a decryptor that finds its own key needs no declared identity");

    assert_eq!(
        values,
        vec![EnvValue::global("TZ", "UTC")],
        "got: {values:?}"
    );
}

#[tokio::test]
async fn a_declared_identity_makes_a_refusal_fatal() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut config = config();
    with_decryptor(&mut config, dir.path(), "exit 1", "exit /b 1");
    write(dir.path(), ENC_FILE_SUFFIX, "sops: {}\n");
    write(dir.path(), ENV_FILE_SUFFIX, "TZ=UTC\n");

    let err = load_global_env(&config, dir.path(), None)
        .await
        .unwrap_err()
        .to_string();

    assert!(
        err.contains("cannot decrypt"),
        "an operator who named an identity meant those values to arrive, got: {err}"
    );
}

#[test]
fn a_config_root_that_is_the_service_directory_has_nowhere_to_stray() {
    let dir = tempfile::tempdir().expect("temp dir");
    assert!(
        stray_global_env(dir.path(), dir.path()).is_none(),
        "one directory cannot hold a misplaced copy of itself"
    );
}

#[test]
fn a_separate_service_directory_is_where_a_shared_file_goes_astray() {
    let root = Path::new("/c/pm3");
    let cfg = Path::new("/c/pm3/service");
    assert_eq!(
        stray_global_env(root, cfg),
        Some(cfg.join(GLOBAL_ENV_STEM)),
        "the operator would naturally drop it beside the service files"
    );
}

#[tokio::test]
async fn a_shared_file_left_in_the_service_directory_is_reported() {
    let dir = tempfile::tempdir().expect("temp dir");
    let cfg = dir.path().join("service");
    std::fs::create_dir_all(&cfg).expect("create the service directory");
    std::fs::write(
        cfg.join(format!("{GLOBAL_ENV_STEM}.{ENV_FILE_SUFFIX}")),
        "TZ=UTC\n",
    )
    .expect("write the misplaced shared environment");

    warn_misplaced_global_env(dir.path(), &cfg).await;

    let values = load_global_env(&config(), dir.path(), None)
        .await
        .expect("the config root holds no shared environment");
    assert!(
        values.is_empty(),
        "the misplaced file must stay ignored, not silently loaded: {values:?}"
    );
}

#[tokio::test]
async fn a_service_directory_without_a_shared_file_reports_nothing() {
    let dir = tempfile::tempdir().expect("temp dir");
    let cfg = dir.path().join("service");
    std::fs::create_dir_all(&cfg).expect("create the service directory");
    warn_misplaced_global_env(dir.path(), &cfg).await;
}

#[tokio::test]
async fn a_single_root_layout_reports_nothing() {
    let dir = tempfile::tempdir().expect("temp dir");
    write(dir.path(), ENV_FILE_SUFFIX, "TZ=UTC\n");
    warn_misplaced_global_env(dir.path(), dir.path()).await;
}
