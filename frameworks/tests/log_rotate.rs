#![cfg(unix)]
#![allow(
    clippy::tests_outside_test_module,
    reason = "integration tests in tests/ are inherently outside #[cfg(test)]"
)]

mod common;

use self::common::{
    app_log, home_with_log_rotate, pm3, shutdown_daemon, stdout_of, wait_for_file, write_apps,
};

#[test]
fn an_oversized_log_is_rotated_aside_and_truncated() {
    let home = home_with_log_rotate(256, 200);
    let cwd = home.root.to_string_lossy();
    let apps = write_apps(
        &home,
        &format!(
            "apps:\n  - name: chatty\n    script: /bin/sh\n    cwd: \"{cwd}\"\n    args:\n      - \"-c\"\n      - \"i=0; while [ $i -lt 200 ]; do echo line; i=$((i+1)); done; exec sleep 30\"\n"
        ),
    );
    let started = pm3(&home, &["start", apps.to_str().expect("path")]);
    assert!(started.status.success(), "{}", stdout_of(&started));

    let backup = home.root.join("logs").join("chatty-out.log.1");
    wait_for_file(&backup);
    wait_for_file(&app_log(&home, "chatty"));
    shutdown_daemon(&home);
}
