#![cfg(target_os = "macos")]
#![allow(
    clippy::tests_outside_test_module,
    reason = "integration tests in tests/ are inherently outside #[cfg(test)]"
)]

mod common;

use std::{
    net::{TcpListener, TcpStream},
    time::Instant,
};

use self::common::{
    Home, PROBE_INTERVAL, READY_BUDGET, app_log, home_with, netcat, shutdown_daemon, wait_for_log,
    workspace_of, write_apps,
};

const LOOPBACK: &str = "127.0.0.1";
const REFUSED_BY_SANDBOX: &str = "Operation not permitted";
const DOCUMENTATION_HOST: &str = "192.0.2.1";

fn free_port() -> u16 {
    TcpListener::bind((LOOPBACK, 0))
        .expect("bind a loopback port")
        .local_addr()
        .expect("read the bound address")
        .port()
}

fn listening_app(home: &Home, name: &str, port: u16, script: &str) -> std::path::PathBuf {
    let cwd = workspace_of(home);
    write_apps(
        home,
        &format!(
            "apps:\n  - name: {name}\n    script: /bin/sh\n    cwd: '{cwd}'\n    autorestart: false\n    args:\n      - \"-c\"\n      - \"{script}\"\n    sandbox:\n      listen:\n        - {port}\n"
        ),
    )
}

fn connects_within_budget(port: u16) -> bool {
    let deadline = Instant::now() + READY_BUDGET;
    while Instant::now() < deadline {
        if TcpStream::connect((LOOPBACK, port)).is_ok() {
            return true;
        }
        std::thread::sleep(PROBE_INTERVAL);
    }
    false
}

#[test]
fn a_listening_app_accepts_a_connection_from_the_host() {
    let home = home_with("workspace-write", false);
    let port = free_port();
    let script = format!("{} -l {LOOPBACK} {port}", netcat());
    let apps = listening_app(&home, "server", port, &script);
    common::start_ok(&home, &apps);

    assert!(
        connects_within_budget(port),
        "the declared port must be reachable from the host"
    );
    shutdown_daemon(&home);
}

#[test]
fn a_listening_app_cannot_dial_another_loopback_port() {
    let home = home_with("workspace-write", false);
    let neighbour = TcpListener::bind((LOOPBACK, 0)).expect("bind a neighbour");
    let target = neighbour.local_addr().expect("neighbour address").port();
    let script = format!(
        "{} -v -z -w 2 {LOOPBACK} {target} 2>&1; echo attempted",
        netcat()
    );
    let apps = listening_app(&home, "dialer", free_port(), &script);
    common::start_ok(&home, &apps);

    let seen = wait_for_log(&app_log(&home, "dialer"), "attempted");
    assert!(
        seen.contains(REFUSED_BY_SANDBOX),
        "a local proxy must stay out of reach: {seen}"
    );
    shutdown_daemon(&home);
}

#[test]
fn a_listening_app_cannot_dial_a_remote_host() {
    let home = home_with("workspace-write", false);
    let script = format!(
        "{} -v -z -w 2 {DOCUMENTATION_HOST} 9 2>&1; echo attempted",
        netcat()
    );
    let apps = listening_app(&home, "roamer", free_port(), &script);
    common::start_ok(&home, &apps);

    let seen = wait_for_log(&app_log(&home, "roamer"), "attempted");
    assert!(
        seen.contains(REFUSED_BY_SANDBOX),
        "the sandbox itself must refuse the connection: {seen}"
    );
    shutdown_daemon(&home);
}

#[test]
fn a_listening_app_cannot_bind_an_undeclared_port() {
    let home = home_with("workspace-write", false);
    let undeclared = free_port();
    let script = format!(
        "{} -l {LOOPBACK} {undeclared} 2>&1; echo attempted",
        netcat()
    );
    let apps = listening_app(&home, "squatter", free_port(), &script);
    common::start_ok(&home, &apps);

    let seen = wait_for_log(&app_log(&home, "squatter"), "attempted");
    assert!(
        seen.contains(REFUSED_BY_SANDBOX),
        "only the declared port may be bound: {seen}"
    );
    shutdown_daemon(&home);
}
