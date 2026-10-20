#![cfg(target_os = "macos")]
#![allow(
    clippy::tests_outside_test_module,
    reason = "integration tests in tests/ are inherently outside #[cfg(test)]"
)]

mod common;

use std::os::unix::fs::PermissionsExt as _;

use self::common::{
    Home, app_error_log, app_log, daemon_pid,
    darwin::{STANDARD_BAND, base_priority, responsible_pid},
    home_with, pm3, shutdown_daemon, start_ok, stderr_of, stdout_of, wait_for_log, workspace_of,
    write_apps,
};

const DISCLAIMED: &str = "self_responsible: true";

fn reporter_apps(home: &Home, name: &str, traits: &str) -> std::path::PathBuf {
    let cwd = workspace_of(home);
    write_apps(
        home,
        &format!(
            "apps:\n  - name: {name}\n    script: /bin/sh\n    cwd: '{cwd}'\n    autorestart: false\n    {traits}\n    args:\n      - \"-c\"\n      - \"echo pid=$$; exec sleep 30\"\n"
        ),
    )
}

fn reported_pid(home: &Home, name: &str) -> libc::pid_t {
    let seen = wait_for_log(&app_log(home, name), "pid=");
    seen.lines()
        .find_map(|line| line.strip_prefix("pid="))
        .and_then(|pid| pid.trim().parse().ok())
        .expect("the app should print its pid")
}

#[test]
fn a_self_responsible_app_answers_for_itself() {
    let home = home_with("danger-full-access", false);
    start_ok(&home, &reporter_apps(&home, "disclaimed", DISCLAIMED));

    let pid = reported_pid(&home, "disclaimed");
    assert_eq!(responsible_pid(pid), pid);
    shutdown_daemon(&home);
}

#[test]
fn one_launch_both_disclaims_and_raises_the_priority() {
    let home = home_with("danger-full-access", false);
    let traits = format!("{DISCLAIMED}\n    priority: interactive");
    start_ok(&home, &reporter_apps(&home, "snappy", &traits));

    let pid = reported_pid(&home, "snappy");
    assert_eq!(responsible_pid(pid), pid);
    assert!(
        base_priority(pid) > STANDARD_BAND,
        "pri {}",
        base_priority(pid)
    );
    shutdown_daemon(&home);
}

#[test]
fn an_ordinary_app_inherits_the_daemon_as_its_responsible_process() {
    let home = home_with("danger-full-access", false);
    start_ok(
        &home,
        &reporter_apps(&home, "inherited", "self_responsible: false"),
    );

    let pid = reported_pid(&home, "inherited");
    let daemon = libc::pid_t::try_from(daemon_pid(&home)).expect("a pid fits");
    assert_eq!(responsible_pid(pid), responsible_pid(daemon));
    shutdown_daemon(&home);
}

fn unloadable(home: &Home) -> std::path::PathBuf {
    let garbage = home.root.join("garbage");
    std::fs::write(&garbage, "not a binary").expect("write");
    std::fs::set_permissions(&garbage, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    garbage
}

#[test]
fn the_trampoline_fails_loudly_when_it_cannot_become_the_program() {
    let home = home_with("danger-full-access", false);
    let garbage = unloadable(&home);
    let ran = std::process::Command::new(common::PM3)
        .args(["__disclaim", "--"])
        .arg(&garbage)
        .output()
        .expect("run the trampoline");

    assert!(!ran.status.success());
    assert!(
        stderr_of(&ran).contains("with its macOS spawn attributes"),
        "{}",
        stderr_of(&ran)
    );
}

#[test]
fn a_program_the_kernel_cannot_load_is_reported_in_the_error_log() {
    let home = home_with("danger-full-access", false);
    let cwd = workspace_of(&home);
    let garbage = unloadable(&home);
    let script = garbage.to_string_lossy();
    let apps = write_apps(
        &home,
        &format!(
            "apps:\n  - name: broken\n    script: '{script}'\n    cwd: '{cwd}'\n    autorestart: false\n    self_responsible: true\n"
        ),
    );
    start_ok(&home, &apps);

    wait_for_log(
        &app_error_log(&home, "broken"),
        "with its macOS spawn attributes",
    );
    shutdown_daemon(&home);
}

#[test]
fn a_confined_app_cannot_be_self_responsible() {
    let home = home_with("workspace-write", false);
    let apps = reporter_apps(&home, "confined", DISCLAIMED);
    let started = pm3(&home, &["start", apps.to_str().expect("path")]);

    assert!(!started.status.success(), "{}", stdout_of(&started));
    assert!(
        stderr_of(&started).contains("self-responsible"),
        "{}",
        stderr_of(&started)
    );
    shutdown_daemon(&home);
}
