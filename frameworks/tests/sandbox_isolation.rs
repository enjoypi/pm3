#![allow(
    clippy::tests_outside_test_module,
    reason = "integration tests in tests/ are inherently outside #[cfg(test)]"
)]

mod common;

use self::common::{
    Home, app_error_log, app_log, home_with_sandbox, netcat, pm3, shutdown_daemon, stdout_of,
    wait_for_file, wait_for_log, write_apps,
};

const OUTSIDE_TARGET: &str = "/pm3-sandbox-escape-probe";

fn shell_app(home: &Home, name: &str, script: &str) -> std::path::PathBuf {
    let cwd = home.root.to_string_lossy();
    write_apps(
        home,
        &format!(
            "apps:\n  - name: {name}\n    script: /bin/sh\n    cwd: \"{cwd}\"\n    autorestart: false\n    args:\n      - \"-c\"\n      - \"{script}\"\n"
        ),
    )
}

#[test]
fn a_confined_app_can_write_inside_its_working_directory() {
    let home = home_with_sandbox("workspace-write", false);
    let apps = shell_app(&home, "writer", "echo inside > ./inside.txt; echo done");
    let started = pm3(&home, &["start", apps.to_str().expect("path")]);
    assert!(started.status.success(), "{}", stdout_of(&started));

    wait_for_log(&app_log(&home, "writer"), "done");
    assert!(
        home.root.join("inside.txt").is_file(),
        "the app should write inside its own cwd"
    );
    shutdown_daemon(&home);
}

#[test]
fn a_confined_app_cannot_write_outside_its_working_directory() {
    let home = home_with_sandbox("workspace-write", false);
    let script = format!("echo escaped > {OUTSIDE_TARGET} 2>&1; echo attempted");
    let apps = shell_app(&home, "escaper", &script);
    let started = pm3(&home, &["start", apps.to_str().expect("path")]);
    assert!(started.status.success(), "{}", stdout_of(&started));

    wait_for_file(&app_log(&home, "escaper"));
    wait_for_log(&app_log(&home, "escaper"), "attempted");
    assert!(
        !std::path::Path::new(OUTSIDE_TARGET).exists(),
        "the sandbox must deny writes outside the workspace"
    );
    shutdown_daemon(&home);
}

#[test]
fn a_confined_app_cannot_reach_the_network() {
    let home = home_with_sandbox("workspace-write", false);
    let script = format!(
        "{} -z -w 2 1.1.1.1 443 && echo reached || echo blocked",
        netcat()
    );
    let apps = shell_app(&home, "dialer", &script);
    let started = pm3(&home, &["start", apps.to_str().expect("path")]);
    assert!(started.status.success(), "{}", stdout_of(&started));

    let seen = wait_for_log(&app_log(&home, "dialer"), "blocked");
    assert!(
        !seen.contains("reached"),
        "the sandbox must deny outbound connections: {seen}"
    );
    shutdown_daemon(&home);
}

#[test]
fn a_confined_app_can_write_through_the_cwd_placeholder() {
    let home = home_with_sandbox("workspace-write", false);
    let started = pm3(
        &home,
        &[
            "start",
            "--name",
            "toucher",
            "/bin/sh",
            "-c",
            "cd /; echo hi > \"$0/probe.txt\" && echo done || echo failed",
            "PM3_SVC_CWD",
        ],
    );
    assert!(started.status.success(), "{}", stdout_of(&started));

    let seen = wait_for_log(&app_log(&home, "toucher"), "done");
    assert!(!seen.contains("failed"), "{seen}");
    assert!(
        home.root.join("toucher").join("probe.txt").is_file(),
        "the placeholder must expand to the very writable root the sandbox allows"
    );
    shutdown_daemon(&home);
}

#[test]
fn an_unconfined_app_keeps_full_access() {
    let home = home_with_sandbox("danger-full-access", true);
    let apps = shell_app(&home, "trusted", "echo trusted > ./trusted.txt; echo done");
    let started = pm3(&home, &["start", apps.to_str().expect("path")]);
    assert!(started.status.success(), "{}", stdout_of(&started));

    wait_for_log(&app_log(&home, "trusted"), "done");
    assert!(
        home.root.join("trusted.txt").is_file(),
        "an unconfined app should write freely"
    );
    assert!(
        app_error_log(&home, "trusted").exists(),
        "the error log should be created next to the stdout log"
    );
    shutdown_daemon(&home);
}
