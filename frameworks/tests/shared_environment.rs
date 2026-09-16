#![cfg(unix)]
#![allow(
    clippy::tests_outside_test_module,
    reason = "integration tests in tests/ are inherently outside #[cfg(test)]"
)]

mod common;

use std::path::PathBuf;

use self::common::{
    Home, app_log, daemon_log, detach_daemon, home, pm3, shutdown_daemon, stdout_of, wait_for_file,
    wait_for_log, write_apps,
};

const NAME: &str = "echoer";
const SHARED_KEY: &str = "SHARED_TZ";
const SHARED_VALUE: &str = "Antarctica/Troll";
const OWN_VALUE: &str = "Asia/Shanghai";
const SHARED_STEM: &str = "pm3";
const PLAIN_SUFFIX: &str = "env";
const SEALED_SUFFIX: &str = "enc.yaml";

fn service_dir(home: &Home) -> PathBuf {
    home.root.join("service")
}

fn shared_file(home: &Home, suffix: &str) -> PathBuf {
    home.root.join(format!("{SHARED_STEM}.{suffix}"))
}

fn own_file(home: &Home) -> PathBuf {
    service_dir(home).join(format!("{NAME}.{PLAIN_SUFFIX}"))
}

fn seen_file(home: &Home) -> PathBuf {
    home.root.join(NAME).join("seen.txt")
}

fn prepare(home: &Home) {
    std::fs::create_dir_all(service_dir(home)).expect("prepare the service directory");
}

fn write_shared(home: &Home, suffix: &str, body: &str) {
    std::fs::write(shared_file(home, suffix), body).expect("write the shared environment");
}

fn echoing_app(home: &Home) -> PathBuf {
    write_apps(
        home,
        &format!(
            "apps:\n  - name: {NAME}\n    script: /bin/sh\n    autorestart: false\n    args:\n      - \"-c\"\n      - \"printf '%s' \\\"${SHARED_KEY}\\\" > seen.txt; echo done\"\n"
        ),
    )
}

fn seen_value(home: &Home) -> String {
    std::fs::read_to_string(seen_file(home)).unwrap_or_default()
}

fn start_and_wait(home: &Home) {
    let apps = echoing_app(home);
    let started = pm3(home, &["start", apps.to_str().expect("path")]);
    assert!(started.status.success(), "{}", stdout_of(&started));
    wait_for_log(&app_log(home, NAME), "done");
}

#[test]
fn a_shared_value_reaches_an_app_that_declares_nothing() {
    let home = home();
    prepare(&home);
    write_shared(
        &home,
        PLAIN_SUFFIX,
        &format!("{SHARED_KEY}={SHARED_VALUE}\n"),
    );

    start_and_wait(&home);

    assert_eq!(
        seen_value(&home),
        SHARED_VALUE,
        "every app inherits the shared values"
    );
    shutdown_daemon(&home);
}

#[test]
fn an_app_value_wins_over_the_shared_one() {
    let home = home();
    prepare(&home);
    write_shared(
        &home,
        PLAIN_SUFFIX,
        &format!("{SHARED_KEY}={SHARED_VALUE}\n"),
    );
    start_and_wait(&home);
    shutdown_daemon(&home);

    std::fs::write(own_file(&home), format!("{SHARED_KEY}={OWN_VALUE}\n"))
        .expect("write the app environment");
    std::fs::remove_file(seen_file(&home)).expect("clear the probe output");

    let restarted = pm3(&home, &["restart", NAME]);
    assert!(restarted.status.success(), "{}", stdout_of(&restarted));
    wait_for_file(&seen_file(&home));

    assert_eq!(
        seen_value(&home),
        OWN_VALUE,
        "an app that spells a value out owns it"
    );
    shutdown_daemon(&home);
}

#[test]
fn an_unreadable_shared_environment_keeps_the_daemon_from_starting() {
    let home = home();
    prepare(&home);
    write_shared(&home, PLAIN_SUFFIX, &format!("{SHARED_KEY}\n"));

    let refused = pm3(&home, &["list"]);

    assert!(
        !refused.status.success(),
        "pm3 must not run every service without the values they inherit: {}",
        stdout_of(&refused)
    );
    let complaint = String::from_utf8_lossy(&refused.stderr);
    let logged = std::fs::read_to_string(daemon_log(&home)).unwrap_or_default();
    assert!(
        complaint.contains("expected KEY=VALUE") || logged.contains("expected KEY=VALUE"),
        "the refusal must name the parse failure, got: {complaint}"
    );
}

#[test]
fn a_sealed_shared_environment_still_lets_every_app_start() {
    let home = home();
    prepare(&home);
    write_shared(&home, SEALED_SUFFIX, "sops: {}\n");
    write_shared(
        &home,
        PLAIN_SUFFIX,
        &format!("{SHARED_KEY}={SHARED_VALUE}\n"),
    );

    start_and_wait(&home);

    assert_eq!(
        seen_value(&home),
        SHARED_VALUE,
        "a sealed sidecar falls back to the plain file"
    );
    shutdown_daemon(&home);
}

#[test]
fn a_shared_value_never_evicts_an_app_across_a_handover() {
    let home = home();
    prepare(&home);
    write_shared(
        &home,
        PLAIN_SUFFIX,
        &format!("{SHARED_KEY}={SHARED_VALUE}\n"),
    );
    let apps = write_apps(
        &home,
        &format!(
            "apps:\n  - name: {NAME}\n    script: /bin/sh\n    args:\n      - \"-c\"\n      - \"echo up; sleep 300\"\n"
        ),
    );
    let started = pm3(&home, &["start", apps.to_str().expect("path")]);
    assert!(started.status.success(), "{}", stdout_of(&started));
    wait_for_log(&app_log(&home, NAME), "up");
    let before = common::described_pid(&home, NAME);

    detach_daemon(&home);
    write_shared(&home, PLAIN_SUFFIX, &format!("{SHARED_KEY}={OWN_VALUE}\n"));
    let listed = pm3(&home, &["list"]);
    assert!(listed.status.success(), "{}", stdout_of(&listed));

    assert_eq!(
        common::described_pid(&home, NAME),
        before,
        "a shared value is deployment context, so changing it must not restart a healthy app"
    );
    let _ = pm3(&home, &["shutdown", "--with-services"]);
}
