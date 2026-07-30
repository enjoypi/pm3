#![allow(
    clippy::tests_outside_test_module,
    reason = "integration tests in tests/ are inherently outside #[cfg(test)]"
)]

mod common;

use std::time::Duration;

use self::common::{daemon_pid, home, pm3, signal, stdout_of, wait_for_file, wait_until_gone};

const SIGNAL_SETTLE: Duration = Duration::from_millis(300);

#[test]
fn the_daemon_swallows_sigint_and_stops_on_sigterm() {
    let home = home();
    let started = pm3(&home, &["list"]);
    assert!(started.status.success(), "{}", stdout_of(&started));
    wait_for_file(&home.root.join("pm3.pid"));
    let pid = daemon_pid(&home);

    signal(pid, "-INT");
    std::thread::sleep(SIGNAL_SETTLE);
    let after_interrupt = pm3(&home, &["list"]);
    assert!(
        after_interrupt.status.success(),
        "the daemon must survive SIGINT: {}",
        stdout_of(&after_interrupt)
    );
    assert_eq!(daemon_pid(&home), pid, "the same daemon should still serve");

    signal(pid, "-TERM");
    wait_until_gone(&home.root.join("pm3.sock"));
    assert!(
        !home.root.join("pm3.pid").exists(),
        "the daemon should clear its pid file on the way out"
    );
}
