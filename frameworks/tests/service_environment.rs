#![cfg(unix)]
#![allow(
    clippy::tests_outside_test_module,
    reason = "integration tests in tests/ are inherently outside #[cfg(test)]"
)]

mod common;

use std::{
    os::unix::fs::PermissionsExt as _,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use self::common::{
    Home, PM3, daemon_log, described_pid, home, pm3, process_is_alive, shutdown_daemon, stdout_of,
    wait_for_file, wait_for_listing, wait_for_log, wait_until_gone, write_apps,
};

const NAME: &str = "keeper";
const TOKEN_KEY: &str = "TUNNEL_TOKEN";
const FIRST_TOKEN: &str = "eyJhIjoiZjQ2NzE0";
const SECOND_TOKEN: &str = "eyJhIjoiYjkwMmMz";
const READABLE_MODE: u32 = 0o644;
const OWNER_ONLY_MODE: u32 = 0o600;
const CONTENT_BUDGET: Duration = Duration::from_secs(10);
const CONTENT_PAUSE: Duration = Duration::from_millis(50);

fn service_file(home: &Home) -> PathBuf {
    home.root.join("service").join(format!("{NAME}.yaml"))
}

fn env_file(home: &Home) -> PathBuf {
    home.root.join("service").join(format!("{NAME}.env"))
}

fn token_file(home: &Home) -> PathBuf {
    home.root.join(NAME).join("token.txt")
}

fn enc_file(home: &Home) -> PathBuf {
    home.root.join("service").join(format!("{NAME}.enc.yaml"))
}

fn decryptor_veto(home: &Home) -> PathBuf {
    home.root.join("decryptor-veto")
}

fn with_decryptor(home: &Home, token: &str) {
    let program = home.root.join("fake-sops");
    std::fs::write(
        &program,
        format!(
            "#!/bin/sh\nif [ -f {} ]; then exit 4; fi\nprintf '{TOKEN_KEY}={token}\\n'\n",
            decryptor_veto(home).to_string_lossy()
        ),
    )
    .expect("write the decryptor stub");
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755))
        .expect("make the decryptor stub executable");
    let identity = home.root.join("age-key");
    std::fs::write(&identity, "fake identity\n").expect("write the identity file");
    let text = std::fs::read_to_string(&home.config).expect("the pm3 config");
    let patched = text.replacen(
        "pm3:\n",
        &format!(
            "pm3:\n  sops_identity_file: \"{}\"\n  sops_program: \"{}\"\n  sops_timeout_ms: 2000\n",
            identity.to_string_lossy(),
            program.to_string_lossy()
        ),
        1,
    );
    std::fs::write(&home.config, patched).expect("rewrite the pm3 config");
    std::fs::create_dir_all(home.root.join("service")).expect("create the service directory");
    std::fs::write(
        enc_file(home),
        format!("{TOKEN_KEY}: ENC[AES256_GCM,data:x]\n"),
    )
    .expect("write the encrypted environment");
}

fn declare_token(home: &Home, token: &str) {
    let path = env_file(home);
    std::fs::create_dir_all(path.parent().expect("the service directory"))
        .expect("create the service directory");
    std::fs::write(
        &path,
        format!("# the tunnel credential\n{TOKEN_KEY}={token}\n"),
    )
    .expect("write the environment file");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(READABLE_MODE))
        .expect("loosen the environment file");
}

fn start_reporter_of(home: &Home, variable: &str) -> std::process::Output {
    let apps = write_apps(
        home,
        &format!(
            "apps:\n  - name: {NAME}\n    script: /bin/sh\n    args:\n      - \"-c\"\n      - \"printf %s \\\"${variable}\\\" > ${{PM3_SERVICE_CWD}}/token.txt; exec sleep 30\"\n"
        ),
    );
    pm3(home, &["start", apps.to_str().expect("path")])
}

fn start_reporter(home: &Home) -> std::process::Output {
    start_reporter_of(home, TOKEN_KEY)
}

fn wait_for_content(path: &Path, expected: &str) -> String {
    let deadline = Instant::now() + CONTENT_BUDGET;
    let mut seen = String::new();
    while Instant::now() < deadline {
        seen = std::fs::read_to_string(path).unwrap_or_default();
        if seen == expected {
            return seen;
        }
        std::thread::sleep(CONTENT_PAUSE);
    }
    seen
}

fn mode_of(path: &Path) -> u32 {
    std::fs::metadata(path)
        .expect("read the metadata")
        .permissions()
        .mode()
        & 0o777
}

#[test]
fn an_environment_file_reaches_the_managed_process() {
    let home = home();
    declare_token(&home, FIRST_TOKEN);

    let started = start_reporter(&home);
    assert!(started.status.success(), "{}", stdout_of(&started));
    wait_for_file(&token_file(&home));
    assert_eq!(
        wait_for_content(&token_file(&home), FIRST_TOKEN),
        FIRST_TOKEN,
        "the managed process must see the declared credential"
    );
    shutdown_daemon(&home);
}

#[test]
fn the_credential_never_lands_in_the_service_file() {
    let home = home();
    declare_token(&home, FIRST_TOKEN);

    let started = start_reporter(&home);
    assert!(started.status.success(), "{}", stdout_of(&started));
    let declaration = std::fs::read_to_string(service_file(&home)).expect("the service file");
    assert!(
        !declaration.contains(FIRST_TOKEN),
        "the service file must stay free of credentials: {declaration}"
    );
    assert!(!declaration.contains("env:"), "got: {declaration}");
    let described = stdout_of(&pm3(&home, &["describe", NAME]));
    assert!(!described.contains(FIRST_TOKEN), "{described}");
    shutdown_daemon(&home);
}

#[test]
fn loading_an_environment_file_tightens_its_permissions() {
    let home = home();
    declare_token(&home, FIRST_TOKEN);
    assert_eq!(mode_of(&env_file(&home)), READABLE_MODE);

    let started = start_reporter(&home);
    assert!(started.status.success(), "{}", stdout_of(&started));
    wait_for_file(&token_file(&home));
    assert_eq!(mode_of(&env_file(&home)), OWNER_ONLY_MODE);
    shutdown_daemon(&home);
}

#[test]
fn restarting_an_app_picks_up_a_rotated_credential() {
    let home = home();
    declare_token(&home, FIRST_TOKEN);
    let started = start_reporter(&home);
    assert!(started.status.success(), "{}", stdout_of(&started));
    wait_for_file(&token_file(&home));

    declare_token(&home, SECOND_TOKEN);
    std::fs::remove_file(token_file(&home)).expect("drop the earlier report");
    let restarted = pm3(&home, &["restart", NAME]);
    assert!(restarted.status.success(), "{}", stdout_of(&restarted));

    assert_eq!(
        wait_for_content(&token_file(&home), SECOND_TOKEN),
        SECOND_TOKEN,
        "a restart must read the rotated credential from disk"
    );
    shutdown_daemon(&home);
}

#[test]
fn pm3_hands_the_home_to_a_managed_process() {
    let home = home();
    let started = start_reporter_of(&home, "HOME");
    assert!(started.status.success(), "{}", stdout_of(&started));
    wait_for_file(&token_file(&home));

    let host_home = std::env::var("HOME").expect("tests always run with HOME");
    assert_eq!(
        wait_for_content(&token_file(&home), &host_home),
        host_home,
        "a service must not have to spell out an absolute home"
    );
    shutdown_daemon(&home);
}

#[test]
fn a_declared_home_wins_over_the_one_pm3_hands_out() {
    let home = home();
    let chosen = home.root.join("elsewhere");
    std::fs::create_dir_all(home.root.join("service")).expect("create the service directory");
    std::fs::write(
        env_file(&home),
        format!("HOME={}\n", chosen.to_string_lossy()),
    )
    .expect("write the environment file");

    let started = start_reporter_of(&home, "HOME");
    assert!(started.status.success(), "{}", stdout_of(&started));
    wait_for_file(&token_file(&home));

    let declared = chosen.to_string_lossy().into_owned();
    assert_eq!(wait_for_content(&token_file(&home), &declared), declared);
    shutdown_daemon(&home);
}

#[test]
fn deleting_an_app_removes_its_environment_file() {
    let home = home();
    declare_token(&home, FIRST_TOKEN);
    let started = start_reporter(&home);
    assert!(started.status.success(), "{}", stdout_of(&started));
    wait_for_file(&token_file(&home));

    let deleted = pm3(&home, &["delete", NAME]);
    assert!(deleted.status.success(), "{}", stdout_of(&deleted));
    assert!(!env_file(&home).exists(), "a deleted app keeps no secrets");
    assert!(!service_file(&home).exists());
    shutdown_daemon(&home);
}

#[test]
fn an_encrypted_environment_reaches_the_managed_process() {
    let home = home();
    with_decryptor(&home, FIRST_TOKEN);

    let started = start_reporter(&home);
    assert!(started.status.success(), "{}", stdout_of(&started));
    wait_for_file(&token_file(&home));

    assert_eq!(
        wait_for_content(&token_file(&home), FIRST_TOKEN),
        FIRST_TOKEN,
        "a sops sidecar takes the place of the plaintext file"
    );
    let described = stdout_of(&pm3(&home, &["describe", NAME]));
    assert!(!described.contains(FIRST_TOKEN), "{described}");
    shutdown_daemon(&home);
}

#[test]
fn a_decryptor_that_fails_stops_the_takeover_instead_of_evicting() {
    let home = home();
    with_decryptor(&home, FIRST_TOKEN);
    let apps = write_apps(
        &home,
        &format!(
            "apps:\n  - name: {NAME}\n    script: {PM3}\n    cwd: \"{}\"\n    args:\n      - \"__sleep\"\n      - \"120000\"\n",
            home.root.to_string_lossy()
        ),
    );
    let started = pm3(&home, &["start", apps.to_str().expect("path")]);
    assert!(started.status.success(), "{}", stdout_of(&started));
    let survivor = described_pid(&home, NAME);

    let parted = pm3(&home, &["shutdown"]);
    assert!(parted.status.success(), "{}", stdout_of(&parted));
    wait_until_gone(&home.root.join("pm3.sock"));
    std::fs::write(decryptor_veto(&home), "broken\n").expect("veto the decryptor");

    let refused = pm3(&home, &["list"]);

    assert!(
        !refused.status.success(),
        "a daemon that cannot take over must not answer: {}",
        stdout_of(&refused)
    );
    wait_for_log(&daemon_log(&home), "stops reading the state file");
    assert!(
        process_is_alive(survivor),
        "the app the previous daemon left running must survive a decryptor failure"
    );

    std::fs::remove_file(decryptor_veto(&home)).expect("let the decryptor work again");
    wait_for_listing(&home, "online");
    assert_eq!(
        described_pid(&home, NAME),
        survivor,
        "once decryption works the app is reclaimed, not restarted"
    );
    shutdown_daemon(&home);
}

#[test]
fn a_sidecar_pm3_never_opened_is_visible_in_the_listing() {
    let home = home();
    with_decryptor(&home, FIRST_TOKEN);
    let text = std::fs::read_to_string(&home.config).expect("the pm3 config");
    std::fs::write(
        &home.config,
        text.replace(
            &format!(
                "sops_identity_file: \"{}/age-key\"",
                home.root.to_string_lossy()
            ),
            "sops_identity_file: \"\"",
        ),
    )
    .expect("forget the identity");
    std::fs::write(decryptor_veto(&home), "veto\n").expect("make the decryptor refuse");

    let started = start_reporter(&home);
    assert!(started.status.success(), "{}", stdout_of(&started));

    let listed = wait_for_listing(&home, NAME);
    assert!(
        listed.contains("env:sealed"),
        "an app running without the credentials it declares must stand out: {listed}"
    );
    let described = stdout_of(&pm3(&home, &["describe", NAME]));
    assert!(
        described.contains("encrypted, not opened"),
        "got: {described}"
    );
    shutdown_daemon(&home);
}
