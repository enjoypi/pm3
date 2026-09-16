#![cfg(unix)]
use std::os::unix::fs::PermissionsExt as _;

use super::*;

const NAME: &str = "caddy";
const IDENTITY: &str = "/home/dev/.ssh/age-cfg";
const SEARCH_PATH: &str = "/usr/bin:/bin";
const TIMEOUT_MS: u64 = 30000;

fn scripted(body: &str) -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("fake-sops");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write the stub");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
        .expect("make the stub executable");
    (dir, path.to_string_lossy().into_owned())
}

fn enc_path(dir: &tempfile::TempDir) -> std::path::PathBuf {
    let path = enc_file_of(dir.path(), NAME).expect("the name should be safe");
    std::fs::write(&path, "K: ENC[fake]\n").expect("write the encrypted file");
    path
}

fn decryptor<'d>(program: &'d str, extra_env: &'d [(String, String)]) -> Decryptor<'d> {
    Decryptor {
        program,
        identity: IDENTITY,
        search_path: SEARCH_PATH,
        timeout_ms: TIMEOUT_MS,
        extra_env,
    }
}

async fn decrypted(body: &str) -> Result<String, EncFileError> {
    let (dir, program) = scripted(body);
    let path = enc_path(&dir);
    load_enc_file(&decryptor(&program, &[]), &path).await
}

#[test]
fn the_file_sits_beside_the_service_file() {
    let path = enc_file_of(std::path::Path::new("/srv/pm3/service"), NAME)
        .expect("the name should be safe");
    assert_eq!(
        path,
        std::path::Path::new("/srv/pm3/service/caddy.enc.yaml"),
        "the encrypted sidecar takes the service name and the enc.yaml suffix"
    );
}

#[test]
fn an_unsafe_name_never_becomes_a_path() {
    enc_file_of(std::path::Path::new("/srv/pm3/service"), "../escape")
        .expect_err("a traversing name should be refused");
}

#[tokio::test]
async fn a_decrypted_file_yields_its_plain_text() {
    let text = decrypted("printf 'TOKEN=abc\\n'")
        .await
        .expect("the stub should decrypt");
    assert_eq!(
        text, "TOKEN=abc\n",
        "the loader hands back exactly what the decryptor printed"
    );
}

#[tokio::test]
async fn a_value_holding_spaces_survives_the_pipe() {
    let text = decrypted("printf 'A=val with spaces\\n'")
        .await
        .expect("the stub should decrypt");
    assert_eq!(
        text, "A=val with spaces\n",
        "the loader never lets a shell re-split an unquoted value"
    );
}

#[tokio::test]
async fn the_identity_reaches_the_decryptor_in_both_shapes() {
    let probe = "printf 'SSH=%s AGE=%s\\n'";
    let text = decrypted(&format!(
        "{probe} \"$SOPS_AGE_SSH_PRIVATE_KEY_FILE\" \"$SOPS_AGE_KEY_FILE\""
    ))
    .await
    .expect("the stub should decrypt");
    assert_eq!(
        text,
        format!("SSH={IDENTITY} AGE={IDENTITY}\n"),
        "an age identity and an ssh identity use different variables and sops reads only the matching one, so pm3 hands the path to both"
    );
}

#[tokio::test]
async fn the_encrypted_path_reaches_the_decryptor() {
    let (dir, program) = scripted("printf 'SEEN=%s\\n' \"$4\"");
    let path = enc_path(&dir);
    let text = load_enc_file(&decryptor(&program, &[]), &path)
        .await
        .expect("the stub should decrypt");
    assert_eq!(
        text,
        format!("SEEN={}\n", path.to_string_lossy()),
        "the encrypted file is the last argument, after the output-type flag"
    );
}

#[tokio::test]
async fn a_missing_decryptor_is_reported_as_unavailable() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = enc_path(&dir);
    let refusal = load_enc_file(&decryptor("/nonexistent/pm3-sops", &[]), &path)
        .await
        .expect_err("a missing decryptor should be refused");
    assert!(
        matches!(refusal, EncFileError::Unavailable { .. }),
        "a decryptor that cannot be spawned is Unavailable, got {refusal:?}"
    );
}

#[tokio::test]
async fn a_stalled_decryptor_is_reported_as_stalled() {
    let (dir, program) = scripted("sleep 5");
    let path = enc_path(&dir);
    let refusal = load_enc_file(
        &Decryptor {
            timeout_ms: 50,
            ..decryptor(&program, &[])
        },
        &path,
    )
    .await
    .expect_err("a stalled decryptor should be refused");
    assert!(
        matches!(refusal, EncFileError::Stalled { .. }),
        "a decryptor past its deadline is Stalled, got {refusal:?}"
    );
}

#[tokio::test]
async fn a_refusing_decryptor_reports_only_its_exit_code() {
    let refusal = decrypted("printf 'AGE-SECRET-KEY-1LEAK\\n' >&2; exit 3")
        .await
        .expect_err("a non-zero exit should be refused");
    let shown = refusal.to_string();
    assert!(
        shown.contains('3'),
        "the exit code identifies the failure, got {shown}"
    );
    assert!(
        !shown.contains("AGE-SECRET-KEY"),
        "stderr may carry key material, so it MUST NOT reach the error text, got {shown}"
    );
}

#[tokio::test]
async fn a_signalled_decryptor_is_still_refused() {
    let refusal = decrypted("kill -TERM $$")
        .await
        .expect_err("a signalled decryptor should be refused");
    assert!(
        matches!(refusal, EncFileError::Refused { .. }),
        "a decryptor killed by a signal is Refused, got {refusal:?}"
    );
}

#[tokio::test]
async fn a_non_utf8_payload_is_refused() {
    let refusal = decrypted("printf 'K=\\300\\300\\n'")
        .await
        .expect_err("invalid utf-8 should be refused");
    assert!(
        matches!(refusal, EncFileError::Unreadable { .. }),
        "a payload that is not text is Unreadable, got {refusal:?}"
    );
}

#[tokio::test]
async fn a_refusal_names_the_file_it_could_not_decrypt() {
    let (dir, program) = scripted("exit 1");
    let path = enc_path(&dir);
    let refusal = load_enc_file(&decryptor(&program, &[]), &path)
        .await
        .expect_err("a non-zero exit should be refused");
    assert!(
        refusal.to_string().contains(&*path.to_string_lossy()),
        "the path is the only locator the operator needs, got {refusal}"
    );
}

#[tokio::test]
async fn the_search_path_reaches_the_decryptor() {
    let text = decrypted("printf 'SEEN=%s\\n' \"$PATH\"")
        .await
        .expect("the stub should decrypt");
    assert_eq!(
        text,
        format!("SEEN={SEARCH_PATH}\n"),
        "pm3 decides where the decryptor is found, so it hands over its own search path"
    );
}

#[tokio::test]
async fn the_decryptor_carries_nothing_but_its_identity_and_path() {
    let probe = "printf 'IDENTITY=[%s] HOME=[%s]\\n'";
    let text = decrypted(&format!(
        "{probe} \"$SOPS_AGE_SSH_PRIVATE_KEY_FILE\" \"$HOME\""
    ))
    .await
    .expect("the stub should decrypt");
    assert_eq!(
        text,
        format!("IDENTITY=[{IDENTITY}] HOME=[]\n"),
        "the identity reaches the decryptor and the parent's own environment does not"
    );
}

#[tokio::test]
async fn a_missing_sidecar_is_absent_rather_than_a_failure() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = enc_file_of(dir.path(), NAME).expect("the name should be safe");
    assert!(
        !enc_file_present(&path)
            .await
            .expect("a missing sidecar is not a failure"),
        "a service without an encrypted sidecar simply has none"
    );
}

#[tokio::test]
async fn a_written_sidecar_is_seen() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = enc_path(&dir);
    assert!(
        enc_file_present(&path).await.expect("the sidecar is there"),
        "a written sidecar must be seen"
    );
}

#[tokio::test]
async fn a_sidecar_pm3_cannot_look_at_is_refused() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, "not a directory").expect("seed the blocking file");
    let refusal = enc_file_present(&blocker.join("web.enc.yaml"))
        .await
        .expect_err("an unreadable path must not pass as absent");
    assert!(
        matches!(refusal, EncFileError::Unreachable { .. }),
        "only a missing file is absent, got {refusal:?}"
    );
    assert!(
        refusal.to_string().starts_with("cannot look at"),
        "got: {refusal}"
    );
}

#[test]
fn a_service_name_can_never_shadow_an_encrypted_sidecar() {
    let dir = std::path::Path::new("/srv/pm3/service");
    let sidecar = enc_file_of(dir, NAME).expect("the name should be safe");
    let shadow = crate::service_file_of(dir, &format!("{NAME}.enc"));
    assert!(
        shadow.is_err(),
        "no service file may land on {}, got {shadow:?}",
        sidecar.display()
    );
}

#[tokio::test]
async fn extra_variables_reach_the_decryptor_beside_the_identity() {
    let (dir, program) =
        scripted("printf 'SEEN=%s IDENTITY=%s\\n' \"$XDG_CONFIG_HOME\" \"$SOPS_AGE_KEY_FILE\"");
    let path = enc_path(&dir);
    let extra = vec![("XDG_CONFIG_HOME".to_string(), "/srv/cfg".to_string())];
    let text = load_enc_file(&decryptor(&program, &extra), &path)
        .await
        .expect("the stub should decrypt");
    assert_eq!(
        text,
        format!("SEEN=/srv/cfg IDENTITY={IDENTITY}\n"),
        "an extra variable never displaces the identity"
    );
}

#[tokio::test]
async fn an_extra_variable_cannot_shadow_the_identity() {
    let (dir, program) = scripted("printf 'IDENTITY=%s\\n' \"$SOPS_AGE_KEY_FILE\"");
    let path = enc_path(&dir);
    let extra = vec![("SOPS_AGE_KEY_FILE".to_string(), "/evil/key".to_string())];
    let text = load_enc_file(&decryptor(&program, &extra), &path)
        .await
        .expect("the stub should decrypt");
    assert_eq!(
        text,
        format!("IDENTITY={IDENTITY}\n"),
        "pm3 decides the identity, so no shared file can redirect it"
    );
}
