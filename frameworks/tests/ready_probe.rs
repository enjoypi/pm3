#![allow(
    clippy::tests_outside_test_module,
    reason = "integration tests in tests/ are inherently outside #[cfg(test)]"
)]

mod common;

use self::common::{
    EXEC_SLEEPER, NAP, SHELL, SHELL_FLAG, daemon_log, home, shutdown_daemon, stdout_of,
    wait_for_listing, write_apps,
};

fn probe_exec(command: &str) -> String {
    format!("        - '{SHELL}'\n        - \"{SHELL_FLAG}\"\n        - '{command}'\n")
}

fn probed_apps(
    home: &common::Home,
    probe_args: &str,
    listen_timeout_ms: u64,
) -> std::path::PathBuf {
    let cwd = home.root.to_string_lossy();
    write_apps(
        home,
        &format!(
            "apps:\n  - name: web\n    script: '{SHELL}'\n    cwd: '{cwd}'\n    listen_timeout_ms: {listen_timeout_ms}\n    ready_probe:\n      exec:\n{probe_args}    args:\n      - \"{SHELL_FLAG}\"\n      - '{EXEC_SLEEPER}'\n"
        ),
    )
}

#[test]
fn an_app_with_a_passing_probe_comes_online() {
    let home = home();
    let apps = probed_apps(&home, &probe_exec("exit 0"), 5000);
    common::start_ok(&home, &apps);

    wait_for_listing(&home, "online");
    shutdown_daemon(&home);
}

#[test]
fn an_app_that_never_becomes_ready_is_marked_errored() {
    let home = home();
    let apps = probed_apps(&home, &probe_exec("exit 1"), 500);
    common::start_ok(&home, &apps);

    let shown = wait_for_listing(&home, "errored");
    assert!(shown.contains("web"), "got: {shown}");
    shutdown_daemon(&home);
}

#[test]
fn a_dependent_app_starts_after_its_dependency_is_ready() {
    let home = home();
    let cwd = home.root.to_string_lossy();
    let nap = probe_exec(NAP);
    let apps = write_apps(
        &home,
        &format!(
            "apps:\n  - name: db\n    script: '{SHELL}'\n    cwd: '{cwd}'\n    listen_timeout_ms: 8000\n    ready_probe:\n      exec:\n{nap}    args:\n      - \"{SHELL_FLAG}\"\n      - '{EXEC_SLEEPER}'\n  - name: web\n    script: '{SHELL}'\n    cwd: '{cwd}'\n    depends_on:\n      - db\n    args:\n      - \"{SHELL_FLAG}\"\n      - '{EXEC_SLEEPER}'\n"
        ),
    );
    let started = common::start_ok(&home, &apps);
    assert!(
        stdout_of(&started).contains("queued web"),
        "web should be queued behind the probe: {}",
        stdout_of(&started)
    );

    let shown = wait_for_listing(&home, "online");
    assert!(shown.contains("web"), "got: {shown}");
    let log = std::fs::read_to_string(daemon_log(&home)).expect("read the daemon log");
    let ready_at = log.find("\"ready\"").expect("the ready log line");
    let spawned_after = log
        .match_indices("\"action\":\"spawn\"")
        .any(|(index, _)| index > ready_at);
    assert!(
        spawned_after,
        "web should spawn after db reported ready: {log}"
    );
    shutdown_daemon(&home);
}
