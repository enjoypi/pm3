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
fn a_reset_clears_the_breaker_state() {
    let home = home();
    let apps = crashing_apps(&home);
    let started = pm3(&home, &["start", apps.to_str().expect("path")]);
    assert!(started.status.success(), "{}", stdout_of(&started));
    wait_for_status(&home, "errored");

    let reset = pm3(&home, &["reset", "flapper"]);
    assert!(reset.status.success(), "{}", stdout_of(&reset));
    assert_eq!(stdout_of(&reset).trim(), "reset flapper");

    let described = stdout_of(&pm3(&home, &["describe", "flapper"]));
    assert!(described.contains("stopped"), "got: {described}");
    let restarts = described
        .lines()
        .find(|line| line.starts_with("restarts"))
        .expect("a restarts row")
        .trim_end();
    assert!(restarts.ends_with('0'), "got: {restarts}");
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

fn clean_exit_apps(home: &common::Home, code: i32) -> std::path::PathBuf {
    let cwd = home.root.to_string_lossy();
    write_apps(
        home,
        &format!(
            "apps:\n  - name: flapper\n    script: /bin/sh\n    cwd: \"{cwd}\"\n    min_uptime_ms: 50\n    max_restarts: 1\n    restart_delay_ms: 1\n    stop_exit_codes:\n      - {code}\n    args:\n      - \"-c\"\n      - \"exit {code}\"\n"
        ),
    )
}

#[test]
fn a_listed_exit_code_settles_without_tripping_the_breaker() {
    let home = home();
    let apps = clean_exit_apps(&home, 3);
    let started = pm3(&home, &["start", apps.to_str().expect("path")]);
    assert!(started.status.success(), "{}", stdout_of(&started));

    let described = wait_for_status(&home, "stopped");
    let restarts = described
        .lines()
        .find(|line| line.starts_with("restarts"))
        .expect("a restarts row")
        .trim_end();
    assert!(restarts.ends_with('0'), "got: {restarts}");

    std::thread::sleep(Duration::from_millis(300));
    let again = stdout_of(&pm3(&home, &["describe", "flapper"]));
    assert!(
        again.contains("stopped"),
        "a clean exit must not restart: {again}"
    );
    shutdown_daemon(&home);
}

#[test]
fn an_out_of_range_stop_exit_code_fails_the_start() {
    let home = home();
    let cwd = home.root.to_string_lossy();
    let apps = write_apps(
        &home,
        &format!(
            "apps:\n  - name: flapper\n    script: /bin/sh\n    cwd: \"{cwd}\"\n    stop_exit_codes:\n      - 999\n    args:\n      - \"-c\"\n      - \"exit 3\"\n"
        ),
    );
    let started = pm3(&home, &["start", apps.to_str().expect("path")]);
    assert!(
        !started.status.success(),
        "an out-of-range stop exit code must fail the start"
    );
    shutdown_daemon(&home);
}
