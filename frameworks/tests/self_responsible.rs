#![cfg(target_os = "macos")]
#![allow(
    clippy::tests_outside_test_module,
    reason = "integration tests in tests/ are inherently outside #[cfg(test)]"
)]

mod common;

use std::os::unix::fs::PermissionsExt as _;

use self::common::{
    Home, app_error_log, app_log, daemon_pid, home_with, pm3, shutdown_daemon, start_ok, stderr_of,
    stdout_of, wait_for_log, workspace_of, write_apps,
};

#[expect(
    unsafe_code,
    reason = "the responsible pid is private libSystem API with no binding"
)]
unsafe extern "C" {
    safe fn responsibility_get_pid_responsible_for_pid(pid: libc::pid_t) -> libc::pid_t;
}

fn reporter_apps(home: &Home, name: &str, self_responsible: bool) -> std::path::PathBuf {
    let cwd = workspace_of(home);
    write_apps(
        home,
        &format!(
            "apps:\n  - name: {name}\n    script: /bin/sh\n    cwd: '{cwd}'\n    autorestart: false\n    self_responsible: {self_responsible}\n    args:\n      - \"-c\"\n      - \"echo pid=$$; exec sleep 30\"\n"
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
    start_ok(&home, &reporter_apps(&home, "disclaimed", true));

    let pid = reported_pid(&home, "disclaimed");
    assert_eq!(responsibility_get_pid_responsible_for_pid(pid), pid);
    shutdown_daemon(&home);
}

#[test]
fn an_ordinary_app_inherits_the_daemon_as_its_responsible_process() {
    let home = home_with("danger-full-access", false);
    start_ok(&home, &reporter_apps(&home, "inherited", false));

    let pid = reported_pid(&home, "inherited");
    let daemon = libc::pid_t::try_from(daemon_pid(&home)).expect("a pid fits");
    assert_eq!(
        responsibility_get_pid_responsible_for_pid(pid),
        responsibility_get_pid_responsible_for_pid(daemon)
    );
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
        stderr_of(&ran).contains("as its own responsible process"),
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
        "as its own responsible process",
    );
    shutdown_daemon(&home);
}

#[test]
fn a_confined_app_cannot_be_self_responsible() {
    let home = home_with("workspace-write", false);
    let apps = reporter_apps(&home, "confined", true);
    let started = pm3(&home, &["start", apps.to_str().expect("path")]);

    assert!(!started.status.success(), "{}", stdout_of(&started));
    assert!(
        stderr_of(&started).contains("self-responsible"),
        "{}",
        stderr_of(&started)
    );
    shutdown_daemon(&home);
}
