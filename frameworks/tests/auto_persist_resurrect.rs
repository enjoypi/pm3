#![allow(
    clippy::tests_outside_test_module,
    reason = "integration tests in tests/ are inherently outside #[cfg(test)]"
)]

mod common;

use self::common::{
    daemon_pid, home, pm3, shutdown_daemon, signal, sleeper_apps, stdout_of, wait_for_file,
    wait_until_gone,
};

#[test]
fn a_restarted_daemon_resurrects_the_managed_apps() {
    let home = home();
    let apps = sleeper_apps(&home, "web");
    let started = pm3(&home, &["start", apps.to_str().expect("path")]);
    assert!(started.status.success(), "{}", stdout_of(&started));
    wait_for_file(&home.root.join("dump.yaml"));

    let first_pid = daemon_pid(&home);
    signal(first_pid, "-TERM");
    wait_until_gone(&home.root.join("pm3.sock"));

    let listed = pm3(&home, &["list"]);
    assert!(
        stdout_of(&listed).contains("online"),
        "the app should come back online: {}",
        stdout_of(&listed)
    );
    assert_ne!(
        daemon_pid(&home),
        first_pid,
        "a fresh daemon should own the socket"
    );
    shutdown_daemon(&home);
}

#[test]
fn state_is_persisted_without_an_explicit_save() {
    let home = home();
    let apps = sleeper_apps(&home, "web");
    pm3(&home, &["start", apps.to_str().expect("path")]);
    wait_for_file(&home.root.join("dump.yaml"));
    let dump = std::fs::read_to_string(home.root.join("dump.yaml")).expect("read the dump");
    assert!(dump.contains("web"), "got: {dump}");
    shutdown_daemon(&home);
}
