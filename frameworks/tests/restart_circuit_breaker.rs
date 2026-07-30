#![allow(
    clippy::tests_outside_test_module,
    reason = "integration tests in tests/ are inherently outside #[cfg(test)]"
)]

mod common;

use std::time::{Duration, Instant};

use self::common::{
    PROBE_INTERVAL, READY_BUDGET, home, pm3, shutdown_daemon, stdout_of, write_apps,
};

fn crashing_apps(home: &common::Home) -> std::path::PathBuf {
    let cwd = home.root.to_string_lossy();
    write_apps(
        home,
        &format!(
            "apps:\n  - name: flapper\n    script: /bin/sh\n    cwd: \"{cwd}\"\n    min_uptime_ms: 50\n    max_restarts: 1\n    restart_delay_ms: 1\n    args:\n      - \"-c\"\n      - \"exit 7\"\n"
        ),
    )
}

fn wait_for_status(home: &common::Home, status: &str) -> String {
    let deadline = Instant::now() + READY_BUDGET;
    while Instant::now() < deadline {
        let described = stdout_of(&pm3(home, &["describe", "flapper"]));
        if described.contains(status) {
            return described;
        }
        std::thread::sleep(PROBE_INTERVAL);
    }
    panic!("flapper should reach {status} inside the budget")
}

#[test]
fn a_crash_loop_trips_the_breaker_and_settles_as_errored() {
    let home = home();
    let apps = crashing_apps(&home);
    let started = pm3(&home, &["start", apps.to_str().expect("path")]);
    assert!(started.status.success(), "{}", stdout_of(&started));

    let described = wait_for_status(&home, "errored");
    assert!(
        described.contains("restarts"),
        "describe should report the restart counter: {described}"
    );
    shutdown_daemon(&home);
}

#[test]
fn a_healthy_app_is_not_restarted() {
    let home = home();
    let apps = common::sleeper_apps(&home, "steady");
    pm3(&home, &["start", apps.to_str().expect("path")]);
    std::thread::sleep(Duration::from_millis(300));
    let described = stdout_of(&pm3(&home, &["describe", "steady"]));
    assert!(described.contains("online"), "got: {described}");
    shutdown_daemon(&home);
}
