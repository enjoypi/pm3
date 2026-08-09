#![cfg(unix)]
#![allow(
    clippy::tests_outside_test_module,
    reason = "integration tests in tests/ are inherently outside #[cfg(test)]"
)]

mod common;

use self::common::{SERVICE_LABEL, home, pm3, stdout_of};

#[test]
fn a_service_that_was_never_installed_reads_as_not_installed() {
    let home = home();
    let queried = pm3(&home, &["service"]);
    assert!(queried.status.success(), "{}", stdout_of(&queried));
    assert!(
        stdout_of(&queried).contains(SERVICE_LABEL),
        "{}",
        stdout_of(&queried)
    );
    assert!(
        stdout_of(&queried).contains("not installed"),
        "{}",
        stdout_of(&queried)
    );
}

#[test]
fn a_dry_run_install_prints_the_unit_without_touching_the_host() {
    let home = home();
    let planned = pm3(&home, &["service", "install", "--dry-run"]);
    assert!(planned.status.success(), "{}", stdout_of(&planned));
    let printed = stdout_of(&planned);
    assert!(printed.contains("write "), "{printed}");
    assert!(printed.contains(SERVICE_LABEL), "{printed}");
    assert!(printed.contains("daemon"), "{printed}");
    assert!(
        stdout_of(&pm3(&home, &["service"])).contains("not installed"),
        "a dry run must leave the host alone"
    );
}

#[test]
fn a_unit_can_wait_for_the_network_before_starting() {
    let home = common::home_waiting_for_network();
    let planned = pm3(&home, &["service", "install", "--dry-run"]);
    assert!(planned.status.success(), "{}", stdout_of(&planned));
    assert!(
        stdout_of(&planned).contains("After=network-online.target"),
        "{}",
        stdout_of(&planned)
    );
}
