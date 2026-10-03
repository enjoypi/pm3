#![allow(
    clippy::tests_outside_test_module,
    reason = "integration tests in tests/ are inherently outside #[cfg(test)]"
)]

mod common;

use self::common::{daemon_log, shell_app, wait_for_log, write_apps};

#[test]
fn an_exit_is_recorded_at_info_with_the_code_the_child_returned() {
    let home = common::home();
    let quitter = shell_app(&home, "quitter", "exit 7");
    let apps = write_apps(&home, &format!("apps:\n{quitter}    autorestart: false\n"));
    common::start_ok(&home, &apps);

    let seen = wait_for_log(&daemon_log(&home), "\"action\":\"settled\"");
    assert!(
        seen.contains("\"exit\":\"code\"") && seen.contains("\"exit_code\":7"),
        "an operator running at info must be able to tell why a service settled: {seen}"
    );
    common::shutdown_daemon(&home);
}
