#![allow(
    clippy::tests_outside_test_module,
    reason = "integration tests in tests/ are inherently outside #[cfg(test)]"
)]

mod common;

use self::common::{daemon_pid, home, pm3, shutdown_daemon, stdout_of, wait_for_file};

#[test]
fn an_orphan_socket_file_is_replaced() {
    let home = home();
    let socket = home.root.join("pm3.sock");
    std::fs::write(&socket, "orphan").expect("seed an orphan socket file");

    let listed = pm3(&home, &["list"]);
    assert!(listed.status.success(), "{}", stdout_of(&listed));
    assert!(
        stdout_of(&listed).contains("no apps"),
        "got: {}",
        stdout_of(&listed)
    );
    shutdown_daemon(&home);
}

#[test]
fn a_second_command_reuses_the_running_daemon() {
    let home = home();
    let first = pm3(&home, &["list"]);
    assert!(first.status.success(), "{}", stdout_of(&first));
    wait_for_file(&home.root.join("pm3.pid"));
    let owner = daemon_pid(&home);

    let second = pm3(&home, &["list"]);
    assert!(second.status.success(), "{}", stdout_of(&second));
    assert_eq!(daemon_pid(&home), owner, "no second daemon should start");
    shutdown_daemon(&home);
}

#[test]
fn a_stale_lock_file_does_not_block_a_running_daemon() {
    let home = home();
    pm3(&home, &["list"]);
    wait_for_file(&home.root.join("pm3.pid"));
    std::fs::write(home.root.join("pm3.lock"), "stale").expect("seed a stale lock");

    let listed = pm3(&home, &["list"]);
    assert!(listed.status.success(), "{}", stdout_of(&listed));
    shutdown_daemon(&home);
}
