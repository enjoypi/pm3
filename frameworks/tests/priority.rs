#![cfg(target_os = "macos")]
#![allow(
    clippy::tests_outside_test_module,
    reason = "integration tests in tests/ are inherently outside #[cfg(test)]"
)]

mod common;

use std::time::Instant;

use self::common::{
    Home, PROBE_INTERVAL, READY_BUDGET, app_log, daemon_pid,
    darwin::{BACKGROUND_BAND, STANDARD_BAND, base_priority, thread_priorities},
    home_with, shutdown_daemon, start_ok, wait_for_log, workspace_of, write_apps,
};

const FAMILY: &str = "sleep 30 & echo child=$!; echo pid=$$; wait";

fn family_app(cwd: &str, name: &str, priority: &str) -> String {
    format!(
        "  - name: {name}\n    script: /bin/sh\n    cwd: '{cwd}'\n    autorestart: false\n    priority: {priority}\n    args:\n      - \"-c\"\n      - \"{FAMILY}\"\n"
    )
}

fn reported(home: &Home, name: &str, key: &str) -> libc::pid_t {
    let seen = wait_for_log(&app_log(home, name), "pid=");
    seen.lines()
        .find_map(|line| line.strip_prefix(key))
        .and_then(|pid| pid.trim().parse().ok())
        .expect("the app should print its pids")
}

fn settled_priority(pid: libc::pid_t, settled: fn(i32) -> bool) -> i32 {
    let deadline = Instant::now() + READY_BUDGET;
    loop {
        let priority = base_priority(pid);
        if settled(priority) || Instant::now() >= deadline {
            return priority;
        }
        std::thread::sleep(PROBE_INTERVAL);
    }
}

fn family_priorities(home: &Home, name: &str, settled: fn(i32) -> bool) -> [i32; 2] {
    [
        settled_priority(reported(home, name, "pid="), settled),
        settled_priority(reported(home, name, "child="), settled),
    ]
}

const fn is_background(priority: i32) -> bool {
    priority == BACKGROUND_BAND
}

const fn is_standard(priority: i32) -> bool {
    priority == STANDARD_BAND
}

const fn is_interactive(priority: i32) -> bool {
    priority > STANDARD_BAND
}

#[test]
fn each_priority_reaches_the_service_and_its_children() {
    let home = home_with("danger-full-access", false);
    let cwd = workspace_of(&home);
    let apps = [
        family_app(&cwd, "quiet", "background"),
        family_app(&cwd, "steady", "standard"),
        family_app(&cwd, "snappy", "interactive"),
    ]
    .concat();
    start_ok(&home, &write_apps(&home, &format!("apps:\n{apps}")));

    assert_eq!(
        family_priorities(&home, "quiet", is_background),
        [BACKGROUND_BAND; 2]
    );
    assert_eq!(
        family_priorities(&home, "steady", is_standard),
        [STANDARD_BAND; 2]
    );
    let snappy = family_priorities(&home, "snappy", is_interactive);
    assert!(snappy.iter().all(|pri| *pri > STANDARD_BAND), "{snappy:?}");
    shutdown_daemon(&home);
}

#[test]
fn an_app_without_a_priority_runs_in_the_background() {
    let home = home_with("danger-full-access", false);
    let cwd = workspace_of(&home);
    let apps = format!(
        "apps:\n  - name: plain\n    script: /bin/sh\n    cwd: '{cwd}'\n    autorestart: false\n    args:\n      - \"-c\"\n      - \"{FAMILY}\"\n"
    );
    start_ok(&home, &write_apps(&home, &apps));

    assert_eq!(
        family_priorities(&home, "plain", is_background),
        [BACKGROUND_BAND; 2]
    );
    shutdown_daemon(&home);
}

#[test]
fn a_confined_app_can_run_at_interactive_priority() {
    let home = home_with("workspace-write", false);
    let cwd = workspace_of(&home);
    let apps = family_app(&cwd, "boxed", "interactive");
    start_ok(&home, &write_apps(&home, &format!("apps:\n{apps}")));

    let boxed = family_priorities(&home, "boxed", is_interactive);
    assert!(boxed.iter().all(|pri| *pri > STANDARD_BAND), "{boxed:?}");
    shutdown_daemon(&home);
}

#[test]
fn the_daemon_runs_every_thread_in_the_background() {
    let home = home_with("danger-full-access", false);
    let cwd = workspace_of(&home);
    let apps = family_app(&cwd, "snappy", "interactive");
    start_ok(&home, &write_apps(&home, &format!("apps:\n{apps}")));

    let daemon = libc::pid_t::try_from(daemon_pid(&home)).expect("a pid fits");
    let threads = thread_priorities(daemon);
    assert!(threads.len() > 1, "{threads:?}");
    assert!(
        threads.iter().all(|pri| *pri == BACKGROUND_BAND),
        "{threads:?}"
    );
    let snappy = family_priorities(&home, "snappy", is_interactive);
    assert!(snappy.iter().all(|pri| *pri > STANDARD_BAND), "{snappy:?}");
    shutdown_daemon(&home);
}
