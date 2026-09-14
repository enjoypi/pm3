#![cfg(unix)]
#![allow(
    clippy::tests_outside_test_module,
    reason = "integration tests in tests/ are inherently outside #[cfg(test)]"
)]

mod common;

use self::common::{
    PM3, daemon_log, home_with_liveness_poll, pm3, shutdown_daemon, stdout_of, wait_for_log,
    write_apps,
};

fn unreachable_probe_apps(home: &common::Home) -> std::path::PathBuf {
    let cwd = home.root.to_string_lossy();
    write_apps(
        home,
        &format!(
            "apps:\n  - name: deaf\n    script: {PM3}\n    cwd: \"{cwd}\"\n    liveness_tcp: \"127.0.0.1:1\"\n    args:\n      - \"__sleep\"\n      - \"30000\"\n"
        ),
    )
}

#[test]
fn a_daemon_kicks_off_liveness_sampling_on_boot() {
    let home = home_with_liveness_poll(200);
    let apps = unreachable_probe_apps(&home);
    let started = pm3(&home, &["start", apps.to_str().expect("path")]);
    assert!(started.status.success(), "{}", stdout_of(&started));

    wait_for_log(&daemon_log(&home), "liveness_failure");
    shutdown_daemon(&home);
}
