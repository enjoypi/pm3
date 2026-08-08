#![allow(
    clippy::tests_outside_test_module,
    reason = "integration tests in tests/ are inherently outside #[cfg(test)]"
)]

mod common;

use self::common::{
    daemon_log, home, pm3, shutdown_daemon, stdout_of, verbose_home, wait_for_listing, write_apps,
};

fn probed_apps(
    home: &common::Home,
    probe_args: &str,
    listen_timeout_ms: u64,
) -> std::path::PathBuf {
    let cwd = home.root.to_string_lossy();
    write_apps(
        home,
        &format!(
            "apps:\n  - name: web\n    script: /bin/sh\n    cwd: \"{cwd}\"\n    listen_timeout_ms: {listen_timeout_ms}\n    ready_probe:\n      exec:\n{probe_args}\n    args:\n      - \"-c\"\n      - \"exec sleep 30\"\n"
        ),
    )
}

#[test]
fn an_app_with_a_passing_probe_comes_online() {
    let home = home();
    let apps = probed_apps(&home, "        - \"/usr/bin/true\"", 5000);
    let started = pm3(&home, &["start", apps.to_str().expect("path")]);
    assert!(started.status.success(), "{}", stdout_of(&started));

    wait_for_listing(&home, "online");
    shutdown_daemon(&home);
}

#[test]
fn an_app_that_never_becomes_ready_is_marked_errored() {
    let home = home();
    let apps = probed_apps(&home, "        - \"/usr/bin/false\"", 500);
    let started = pm3(&home, &["start", apps.to_str().expect("path")]);
    assert!(started.status.success(), "{}", stdout_of(&started));

    let shown = wait_for_listing(&home, "errored");
    assert!(shown.contains("web"), "got: {shown}");
    shutdown_daemon(&home);
}

#[test]
fn a_dependent_app_starts_after_its_dependency_is_ready() {
    let home = verbose_home();
    let cwd = home.root.to_string_lossy();
    let apps = write_apps(
        &home,
        &format!(
            "apps:\n  - name: db\n    script: /bin/sh\n    cwd: \"{cwd}\"\n    listen_timeout_ms: 8000\n    ready_probe:\n      exec:\n        - \"/bin/sh\"\n        - \"-c\"\n        - \"sleep 1\"\n    args:\n      - \"-c\"\n      - \"exec sleep 30\"\n  - name: web\n    script: /bin/sh\n    cwd: \"{cwd}\"\n    depends_on:\n      - db\n    args:\n      - \"-c\"\n      - \"exec sleep 30\"\n"
        ),
    );
    let started = pm3(&home, &["start", apps.to_str().expect("path")]);
    assert!(started.status.success(), "{}", stdout_of(&started));
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
