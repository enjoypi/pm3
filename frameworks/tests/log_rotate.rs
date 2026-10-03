#![allow(
    clippy::tests_outside_test_module,
    reason = "integration tests in tests/ are inherently outside #[cfg(test)]"
)]

mod common;

use self::common::{
    app_log, flood_command, home_with_log_rotate, shell_app, shutdown_daemon, wait_for_file,
    write_apps,
};

#[test]
fn an_oversized_log_is_rotated_aside_and_truncated() {
    let home = home_with_log_rotate(256, 200);
    let chatty = shell_app(&home, "chatty", &flood_command(200));
    let apps = write_apps(&home, &format!("apps:\n{chatty}"));
    common::start_ok(&home, &apps);

    let backup = home.root.join("logs").join("chatty-out.log.1");
    wait_for_file(&backup);
    wait_for_file(&app_log(&home, "chatty"));
    shutdown_daemon(&home);
}
