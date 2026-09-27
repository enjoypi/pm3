use super::*;
use crate::spec_sources::{with_decryptor, write_enc_file};

#[tokio::test]
async fn an_encrypted_environment_reaches_the_service() {
    let mut fixture = fixture();
    register_service(&fixture.source, "web");
    write_enc_file(&fixture.source, "web", "TUNNEL_TOKEN: ENC[fake]\n");
    with_decryptor(
        &mut fixture.source,
        "printf 'TUNNEL_TOKEN=eyJhIjoiZjQ2\\n'",
        "echo TUNNEL_TOKEN=eyJhIjoiZjQ2",
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
            EnvValue::app("TUNNEL_TOKEN", "eyJhIjoiZjQ2"),
        ]
    );
}

#[tokio::test]
async fn an_encrypted_environment_wins_over_a_plaintext_one() {
    let mut fixture = fixture();
    register_service(&fixture.source, "web");
    write_env_file(&fixture.source, "web", "SOURCE=plaintext\n");
    write_enc_file(&fixture.source, "web", "SOURCE: ENC[fake]\n");
    with_decryptor(
        &mut fixture.source,
        "printf 'SOURCE=encrypted\\n'",
        "echo SOURCE=encrypted",
    );
    let spec = fixture
        .source
        .resolve_service("web")
        .await
        .expect("the service should resolve");
    assert!(
        spec.env.contains(&EnvValue::app("SOURCE", "encrypted")),
        "the encrypted sidecar is the one that counts, got {:?}",
        spec.env
    );
}

#[tokio::test]
async fn an_encrypted_environment_opens_without_a_declared_identity() {
    let mut fixture = fixture();
    register_service(&fixture.source, "web");
    write_env_file(&fixture.source, "web", "SOURCE=plaintext\n");
    write_enc_file(&fixture.source, "web", "SOURCE: ENC[fake]\n");
    with_decryptor(
        &mut fixture.source,
        "printf 'SOURCE=encrypted\\n'",
        "echo SOURCE=encrypted",
    );
    fixture.source.config.sops_identity_file = String::new();
    let spec = fixture
        .source
        .resolve_service("web")
        .await
        .expect("the service should resolve");
    assert!(
        spec.env.contains(&EnvValue::app("SOURCE", "encrypted")),
        "a decryptor that finds its own key wins over the plain sidecar, got {:?}",
        spec.env
    );
}

#[tokio::test]
async fn a_failing_decryptor_stops_the_service_from_resolving() {
    let mut fixture = fixture();
    register_service(&fixture.source, "web");
    write_enc_file(&fixture.source, "web", "TUNNEL_TOKEN: ENC[fake]\n");
    with_decryptor(&mut fixture.source, "exit 4", "exit /b 4");
    let err = fixture
        .source
        .resolve_service("web")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("exited with status 4"), "got: {err}");
}

#[tokio::test]
async fn an_encrypted_value_may_hold_spaces() {
    let mut fixture = fixture();
    register_service(&fixture.source, "web");
    write_enc_file(&fixture.source, "web", "MOTTO: ENC[fake]\n");
    with_decryptor(
        &mut fixture.source,
        "printf 'MOTTO=two words\\n'",
        "echo MOTTO=two words",
    );
    let spec = fixture
        .source
        .resolve_service("web")
        .await
        .expect("the service should resolve");
    assert!(
        spec.env.contains(&EnvValue::app("MOTTO", "two words")),
        "an unquoted dotenv value keeps its spaces, got {:?}",
        spec.env
    );
}

#[tokio::test]
async fn an_encrypted_environment_expands_the_host_home() {
    let mut fixture = fixture();
    register_service(&fixture.source, "web");
    write_enc_file(&fixture.source, "web", "BIN: ENC[fake]\n");
    with_decryptor(
        &mut fixture.source,
        "printf 'BIN=$HOME/bin\\n'",
        "echo BIN=$HOME/bin",
    );
    let spec = fixture
        .source
        .resolve_service("web")
        .await
        .expect("the service should resolve");
    assert!(
        spec.env
            .contains(&EnvValue::app("BIN", &format!("{HOST_HOME}/bin"))),
        "the decrypted path goes through the same $HOME expansion, got {:?}",
        spec.env
    );
}

#[tokio::test]
async fn a_service_without_an_encrypted_sidecar_still_reads_its_plaintext_one() {
    let mut fixture = fixture();
    register_service(&fixture.source, "web");
    write_env_file(&fixture.source, "web", "SOURCE=plaintext\n");
    with_decryptor(
        &mut fixture.source,
        "printf 'SOURCE=encrypted\\n'",
        "echo SOURCE=encrypted",
    );
    let spec = fixture
        .source
        .resolve_service("web")
        .await
        .expect("the service should resolve");
    assert!(
        spec.env.contains(&EnvValue::app("SOURCE", "plaintext")),
        "a configured identity alone must not conjure an encrypted sidecar, got {:?}",
        spec.env
    );
}

#[tokio::test]
async fn an_unparsable_decrypted_environment_is_reported() {
    let mut fixture = fixture();
    register_service(&fixture.source, "web");
    write_enc_file(&fixture.source, "web", "TUNNEL_TOKEN: ENC[fake]\n");
    with_decryptor(
        &mut fixture.source,
        "printf 'TUNNEL_TOKEN\\n'",
        "echo TUNNEL_TOKEN",
    );
    let err = fixture
        .source
        .resolve_service("web")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("expected KEY=VALUE"), "got: {err}");
}

#[tokio::test]
async fn an_encrypted_value_reaches_the_service_exactly_as_it_was_decrypted() {
    let mut fixture = fixture();
    register_service(&fixture.source, "web");
    write_enc_file(&fixture.source, "web", "PASSWORD: ENC[fake]\n");
    with_decryptor(
        &mut fixture.source,
        "printf 'PASSWORD=\"p@ss\\\\word\"\\n'",
        "echo PASSWORD=\"p@ss\\word\"",
    );
    let spec = fixture
        .source
        .resolve_service("web")
        .await
        .expect("the service should resolve");
    assert!(
        spec.env
            .contains(&EnvValue::app("PASSWORD", "\"p@ss\\word\"")),
        "a decrypted secret is handed over untouched, got {:?}",
        spec.env
    );
}

#[tokio::test]
async fn an_unreachable_encrypted_sidecar_stops_the_service_from_resolving() {
    let mut fixture = fixture();
    register_service(&fixture.source, "web");
    let path = crate::enc_file_of(&fixture.source.cfg_dir, "web").expect("a safe service name");
    crate::platform::link_dir(&path, &path);
    with_decryptor(&mut fixture.source, "printf 'A=b\\n'", "echo A=b");
    let err = fixture
        .source
        .resolve_service("web")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot look at"), "got: {err}");
}

#[tokio::test]
async fn a_decrypted_environment_is_remembered_as_its_own_origin() {
    let mut fixture = fixture();
    register_service(&fixture.source, "web");
    write_enc_file(&fixture.source, "web", "TOKEN: ENC[fake]\n");
    with_decryptor(
        &mut fixture.source,
        "printf 'TOKEN=abc\\n'",
        "echo TOKEN=abc",
    );
    let spec = fixture
        .source
        .resolve_service("web")
        .await
        .expect("the service should resolve");
    assert_eq!(spec.env_origin, usecases::EnvOrigin::Encrypted);
}

#[tokio::test]
async fn a_sidecar_pm3_never_opened_is_remembered_as_sealed() {
    let mut fixture = fixture();
    register_service(&fixture.source, "web");
    write_env_file(&fixture.source, "web", "SOURCE=plaintext\n");
    write_enc_file(&fixture.source, "web", "TOKEN: ENC[fake]\n");
    with_decryptor(&mut fixture.source, "exit 1", "exit /b 1");
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
        spec.env.contains(&EnvValue::app("SOURCE", "plaintext")),
        "got {:?}",
        spec.env
    );
}

#[tokio::test]
async fn a_sealed_sidecar_still_reports_a_broken_plaintext_fallback() {
    let mut fixture = fixture();
    register_service(&fixture.source, "web");
    write_env_file(&fixture.source, "web", "BROKEN\n");
    write_enc_file(&fixture.source, "web", "TOKEN: ENC[fake]\n");
    with_decryptor(
        &mut fixture.source,
        "printf 'TOKEN=abc\\n'",
        "echo TOKEN=abc",
    );
    fixture.source.config.sops_identity_file = String::new();
    let err = fixture
        .source
        .resolve_service("web")
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("expected KEY=VALUE"), "got: {err}");
}

#[tokio::test]
async fn a_decryptor_that_needs_no_identity_still_opens_a_service_secret() {
    let mut fixture = fixture();
    with_decryptor(
        &mut fixture.source,
        "printf 'TOKEN=opened\\n'",
        "echo TOKEN=opened",
    );
    fixture.source.config.sops_identity_file = String::new();
    register_service(&fixture.source, "web");
    write_enc_file(&fixture.source, "web", "TOKEN: ENC[fake]\n");

    let spec = fixture
        .source
        .resolve_service("web")
        .await
        .expect("a decryptor that finds its own key needs no declared identity");

    assert!(
        spec.env.contains(&EnvValue::app("TOKEN", "opened")),
        "got: {:?}",
        spec.env
    );
    assert_eq!(
        spec.env_origin,
        usecases::EnvOrigin::Encrypted,
        "got: {:?}",
        spec.env_origin
    );
}

#[tokio::test]
async fn a_refusal_without_an_identity_leaves_a_service_secret_sealed() {
    let mut fixture = fixture();
    with_decryptor(&mut fixture.source, "exit 1", "exit /b 1");
    fixture.source.config.sops_identity_file = String::new();
    register_service(&fixture.source, "web");
    write_enc_file(&fixture.source, "web", "TOKEN: ENC[fake]\n");
    write_env_file(&fixture.source, "web", "PORT=8080\n");

    let spec = fixture
        .source
        .resolve_service("web")
        .await
        .expect("without a declared identity pm3 only tries, it does not insist");

    assert_eq!(
        spec.env_origin,
        usecases::EnvOrigin::Sealed,
        "got: {:?}",
        spec.env_origin
    );
    assert!(
        spec.env.contains(&EnvValue::app("PORT", "8080")),
        "the plain sidecar still stands, got: {:?}",
        spec.env
    );
}
