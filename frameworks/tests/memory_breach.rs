#![cfg(unix)]
#![allow(
    clippy::tests_outside_test_module,
    reason = "integration tests in tests/ are inherently outside #[cfg(test)]"
)]

mod common;

use self::common::{
    PM3, daemon_log, home_with_memory_poll, pm3, shutdown_daemon, stdout_of, wait_for_log,
    write_apps,
};

fn memory_limited_apps(home: &common::Home) -> std::path::PathBuf {
    let cwd = home.root.to_string_lossy();
    write_apps(
        home,
        &format!(
            "apps:\n  - name: hog\n    script: {PM3}\n    cwd: \"{cwd}\"\n    max_memory: \"1K\"\n    args:\n      - \"__sleep\"\n      - \"30000\"\n"
        ),
    )
}

#[test]
fn a_daemon_kicks_off_memory_sampling_on_boot() {
    let home = home_with_memory_poll(200);
    let apps = memory_limited_apps(&home);
    let started = pm3(&home, &["start", apps.to_str().expect("path")]);
    assert!(started.status.success(), "{}", stdout_of(&started));

    wait_for_log(&daemon_log(&home), "memory_breach");
    shutdown_daemon(&home);
}
