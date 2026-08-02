#![allow(
    clippy::tests_outside_test_module,
    reason = "integration tests in tests/ are inherently outside #[cfg(test)]"
)]

mod common;

use self::common::{
    app_log, daemon_pid, home, pm3, shutdown_daemon, sleeper_apps, stdout_of, wait_for_file,
};

#[test]
fn the_whole_lifecycle_works_through_the_cli() {
    let home = home();
    let apps = sleeper_apps(&home, "web");

    let started = pm3(&home, &["start", apps.to_str().expect("path")]);
    assert!(started.status.success(), "{}", stdout_of(&started));
    assert!(
        stdout_of(&started).contains("started web"),
        "{}",
        stdout_of(&started)
    );

    wait_for_file(&home.root.join("pm3.sock"));
    wait_for_file(&home.root.join("pm3.pid"));
    assert!(daemon_pid(&home) > 0, "the daemon should record its pid");

    let listed = pm3(&home, &["list"]);
    assert!(stdout_of(&listed).contains("web"), "{}", stdout_of(&listed));
    assert!(
        stdout_of(&listed).contains("online"),
        "{}",
        stdout_of(&listed)
    );

    let described = pm3(&home, &["describe", "web"]);
    assert!(
        stdout_of(&described).contains("writable roots"),
        "{}",
        stdout_of(&described)
    );

    wait_for_file(&app_log(&home, "web"));
    let logs = pm3(&home, &["logs", "web", "-n", "5"]);
    assert!(logs.status.success(), "{}", stdout_of(&logs));

    let stopped = pm3(&home, &["stop", "web"]);
    assert_eq!(stdout_of(&stopped).trim(), "stopped web");
    std::thread::sleep(std::time::Duration::from_millis(700));

    let restarted = pm3(&home, &["restart", "web"]);
    assert_eq!(stdout_of(&restarted).trim(), "restarted web");

    let deleted = pm3(&home, &["delete", "web"]);
    assert_eq!(stdout_of(&deleted).trim(), "deleted web");

    let empty = pm3(&home, &["list"]);
    assert!(
        stdout_of(&empty).contains("no apps"),
        "{}",
        stdout_of(&empty)
    );

    shutdown_daemon(&home);
}

#[test]
fn an_unknown_app_fails_the_command() {
    let home = home();
    let described = pm3(&home, &["describe", "ghost"]);
    assert!(!described.status.success(), "an unknown app should fail");
    shutdown_daemon(&home);
}

#[test]
fn config_check_needs_no_daemon() {
    let home = home();
    let checked = pm3(&home, &["config", "check"]);
    assert!(checked.status.success(), "{}", stdout_of(&checked));
    assert!(
        !home.root.join("pm3.sock").exists(),
        "config check must not start a daemon"
    );
}

#[test]
fn a_writable_root_that_does_not_exist_yet_is_accepted() {
    let home = home();
    let cwd = home.root.to_string_lossy();
    let apps = self::common::write_apps(
        &home,
        &format!(
            "apps:\n  - name: web\n    script: /bin/sh\n    cwd: \"{cwd}\"\n    args:\n      - \"-c\"\n      - \"sleep 30\"\n    sandbox:\n      mode: danger-full-access\n      writable_roots:\n        - /nonexistent/pm3-root\n"
        ),
    );
    let started = pm3(&home, &["start", apps.to_str().expect("path")]);
    assert!(started.status.success(), "{}", stdout_of(&started));
    shutdown_daemon(&home);
}

#[test]
fn the_hidden_sleep_target_exits_cleanly() {
    let home = home();
    let slept = pm3(&home, &["__sleep", "10"]);
    assert!(slept.status.success(), "__sleep should exit cleanly");
}

#[test]
fn a_shutdown_force_kills_a_service_that_ignores_the_stop_signal() {
    let home = home();
    let cwd = home.root.to_string_lossy();
    let apps = self::common::write_apps(
        &home,
        &format!(
            "apps:\n  - name: stubborn\n    script: /bin/sh\n    cwd: \"{cwd}\"\n    args:\n      - \"-c\"\n      - \"trap '' TERM; while true; do sleep 1; done\"\n"
        ),
    );
    let started = pm3(&home, &["start", apps.to_str().expect("path")]);
    assert!(started.status.success(), "{}", stdout_of(&started));
    let pid = self::common::described_pid(&home, "stubborn");

    shutdown_daemon(&home);

    let deadline = std::time::Instant::now() + self::common::READY_BUDGET;
    while self::common::process_is_alive(pid) {
        assert!(
            std::time::Instant::now() < deadline,
            "pid {pid} should be force killed"
        );
        std::thread::sleep(self::common::PROBE_INTERVAL);
    }
}

#[test]
fn a_plain_kill_spares_online_services_but_sweeps_a_stuck_stopping_one() {
    let home = home();
    let keeper_apps = sleeper_apps(&home, "keeper");
    let keeper_started = pm3(&home, &["start", keeper_apps.to_str().expect("path")]);
    assert!(
        keeper_started.status.success(),
        "{}",
        stdout_of(&keeper_started)
    );
    let keeper_pid = self::common::described_pid(&home, "keeper");

    let cwd = home.root.to_string_lossy();
    let stubborn_apps = self::common::write_apps(
        &home,
        &format!(
            "apps:\n  - name: stubborn\n    script: /bin/sh\n    cwd: \"{cwd}\"\n    args:\n      - \"-c\"\n      - \"trap '' TERM; while true; do sleep 1; done\"\n"
        ),
    );
    let stubborn_started = pm3(&home, &["start", stubborn_apps.to_str().expect("path")]);
    assert!(
        stubborn_started.status.success(),
        "{}",
        stdout_of(&stubborn_started)
    );
    let stubborn_pid = self::common::described_pid(&home, "stubborn");

    let stopped = pm3(&home, &["stop", "stubborn"]);
    assert_eq!(stdout_of(&stopped).trim(), "stopped stubborn");
    let killed = pm3(&home, &["kill"]);
    assert!(killed.status.success(), "{}", stdout_of(&killed));
    self::common::wait_until_gone(&home.root.join("pm3.sock"));

    let deadline = std::time::Instant::now() + self::common::READY_BUDGET;
    while self::common::process_is_alive(stubborn_pid) {
        assert!(
            std::time::Instant::now() < deadline,
            "pid {stubborn_pid} should be swept after the drain budget"
        );
        std::thread::sleep(self::common::PROBE_INTERVAL);
    }
    assert!(
        self::common::process_is_alive(keeper_pid),
        "a plain kill must preserve online services"
    );
    self::common::signal(keeper_pid, "-TERM");
}
