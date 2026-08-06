#![allow(
    clippy::tests_outside_test_module,
    reason = "integration tests in tests/ are inherently outside #[cfg(test)]"
)]

mod common;

use self::common::{
    Home, MINIMAL_READ, app_log, home_with_read_scope, home_with_sandbox, pm3, shutdown_daemon,
    stdout_of, wait_for_log, workspace_of, write_apps,
};

const LEAKED: &str = "leaked";
const BLOCKED: &str = "blocked";
const VISIBLE: &str = "visible";
const HIDDEN: &str = "hidden";

fn probe_app(home: &Home, name: &str, script: &str) -> std::path::PathBuf {
    let cwd = workspace_of(home);
    write_apps(
        home,
        &format!(
            "apps:\n  - name: {name}\n    script: /bin/sh\n    cwd: \"{cwd}\"\n    autorestart: false\n    args:\n      - \"-c\"\n      - \"{script}\"\n"
        ),
    )
}

fn run_probe(home: &Home, name: &str, script: &str) -> String {
    let apps = probe_app(home, name, script);
    let started = pm3(home, &["start", apps.to_str().expect("path")]);
    assert!(started.status.success(), "{}", stdout_of(&started));
    let seen = wait_for_log(&app_log(home, name), "done");
    shutdown_daemon(home);
    seen
}

#[test]
fn a_confined_app_cannot_read_the_credentials_of_another_service() {
    let home = home_with_sandbox("workspace-write", false);
    let cfg_dir = home.root.join("service");
    std::fs::create_dir_all(&cfg_dir).expect("prepare the service directory");
    let secret = cfg_dir.join("other.env");
    std::fs::write(&secret, "TOKEN=super-secret\n").expect("write the neighbour credentials");

    let script = format!(
        "cat {} >/dev/null 2>&1 && echo {LEAKED} || echo {BLOCKED}; echo done",
        secret.to_string_lossy()
    );
    let seen = run_probe(&home, "peeper", &script);

    assert!(
        seen.contains(BLOCKED) && !seen.contains(LEAKED),
        "one service must never read another service's credentials: {seen}"
    );
}

#[test]
fn a_confined_app_cannot_see_the_daemon_control_socket() {
    let home = home_with_sandbox("workspace-write", false);
    let socket = home.root.join("pm3.sock");
    let script = format!(
        "test -S {} && echo {VISIBLE} || echo {HIDDEN}; echo done",
        socket.to_string_lossy()
    );
    let seen = run_probe(&home, "prober", &script);

    assert!(
        seen.contains(HIDDEN) && !seen.contains(VISIBLE),
        "the control socket must stay out of every sandbox: {seen}"
    );
}

#[test]
fn a_confined_app_still_reads_its_own_working_directory() {
    let home = home_with_sandbox("workspace-write", false);
    let readable = std::path::PathBuf::from(workspace_of(&home)).join("payload.txt");
    std::fs::write(&readable, "payload\n").expect("write the payload");
    let script = format!(
        "cat {} >/dev/null 2>&1 && echo {LEAKED} || echo {BLOCKED}; echo done",
        readable.to_string_lossy()
    );
    let seen = run_probe(&home, "reader", &script);

    assert!(
        seen.contains(LEAKED),
        "masking the pm3 home must not take the workspace with it: {seen}"
    );
}

#[test]
fn a_minimal_read_scope_still_runs_a_shell_script() {
    let home = home_with_read_scope("workspace-write", false, MINIMAL_READ);
    let seen = run_probe(&home, "minimal", "echo done");

    assert!(
        seen.contains("done"),
        "the system allowlist must cover a plain shell: {seen}"
    );
}

#[test]
fn a_minimal_read_scope_hides_a_path_outside_the_allowlist() {
    let home = home_with_read_scope("workspace-write", false, MINIMAL_READ);
    let outside = home.dir.path().join("outside.txt");
    std::fs::write(&outside, "outside\n").expect("write the outside file");
    let script = format!(
        "cat {} >/dev/null 2>&1 && echo {LEAKED} || echo {BLOCKED}; echo done",
        outside.to_string_lossy()
    );
    let seen = run_probe(&home, "narrow", &script);

    assert!(
        seen.contains(BLOCKED) && !seen.contains(LEAKED),
        "a confined read scope must not reach outside its allowlist: {seen}"
    );
}
