#![allow(
    clippy::tests_outside_test_module,
    reason = "integration tests in tests/ are inherently outside #[cfg(test)]"
)]

mod common;

use std::{
    io::{Read as _, Write as _},
    os::unix::net::UnixListener,
    path::{Path, PathBuf},
};

use self::common::{
    Home, daemon_log, described_pid, detach_daemon, home, pm3, process_is_alive, shutdown_daemon,
    stdout_of, verbose_home, wait_for_file, wait_for_log, write_apps,
};

const SERVICE: &str = "keeper";

fn start_sleeper(home: &Home) -> u32 {
    let cwd = home.root.to_string_lossy();
    let apps = write_apps(
        home,
        &format!(
            "apps:\n  - name: {SERVICE}\n    script: {}\n    cwd: \"{cwd}\"\n    args:\n      - \"__sleep\"\n      - \"30000\"\n",
            common::PM3
        ),
    );
    let started = pm3(home, &["start", apps.to_str().expect("path")]);
    assert!(started.status.success(), "{}", stdout_of(&started));
    wait_for_file(&home.root.join("pm3.pid"));
    described_pid(home, SERVICE)
}

fn service_file(home: &Home) -> PathBuf {
    home.root.join("svc").join(format!("{SERVICE}.yaml"))
}

fn revive_daemon(home: &Home) {
    let listed = pm3(home, &["list"]);
    assert!(listed.status.success(), "{}", stdout_of(&listed));
    wait_for_file(&home.root.join("pm3.pid"));
}

fn script_at(dir: &Path, body: &str) -> PathBuf {
    let path = dir.join("service.sh");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write the service script");
    let mode = std::os::unix::fs::PermissionsExt::from_mode(0o755);
    std::fs::set_permissions(&path, mode).expect("make the service script executable");
    path
}

#[test]
fn a_service_outlives_the_daemon_that_launched_it() {
    let home = home();
    let pid = start_sleeper(&home);

    detach_daemon(&home);

    assert!(
        process_is_alive(pid),
        "the service should still run after the daemon left"
    );
    shutdown_daemon(&home);
}

#[test]
fn an_unchanged_service_keeps_its_process_across_a_daemon_restart() {
    let home = verbose_home();
    let pid = start_sleeper(&home);

    detach_daemon(&home);
    revive_daemon(&home);

    assert_eq!(
        described_pid(&home, SERVICE),
        pid,
        "the new daemon should reclaim the very same process"
    );
    shutdown_daemon(&home);
}

#[test]
fn reclaiming_a_service_is_reported_in_the_daemon_log() {
    let home = verbose_home();
    start_sleeper(&home);

    detach_daemon(&home);
    revive_daemon(&home);

    let log = wait_for_log(&daemon_log(&home), "\"action\":\"adopt\"");
    assert!(
        log.contains(&format!("\"service\":\"{SERVICE}\"")),
        "got: {log}"
    );
    shutdown_daemon(&home);
}

#[test]
fn a_reclaimed_service_is_still_reported_as_online() {
    let home = home();
    start_sleeper(&home);

    detach_daemon(&home);
    revive_daemon(&home);

    let listed = pm3(&home, &["list"]);
    assert!(
        stdout_of(&listed).contains("online"),
        "{}",
        stdout_of(&listed)
    );
    shutdown_daemon(&home);
}

#[test]
fn a_service_whose_config_changed_is_restarted_by_the_new_daemon() {
    let home = verbose_home();
    let pid = start_sleeper(&home);

    detach_daemon(&home);
    let path = service_file(&home);
    let config = std::fs::read_to_string(&path).expect("the service file");
    std::fs::write(&path, format!("{config}env:\n  TUNED: \"1\"\n")).expect("retune the service");
    revive_daemon(&home);

    assert_ne!(
        described_pid(&home, SERVICE),
        pid,
        "a retuned service must not be reclaimed"
    );
    let log = wait_for_log(&daemon_log(&home), "\"action\":\"respawn\"");
    assert!(log.contains("\"reason\":\"launch\""), "got: {log}");
    shutdown_daemon(&home);
}

#[test]
fn restarting_a_changed_service_takes_the_old_process_down_first() {
    let home = verbose_home();
    let pid = start_sleeper(&home);

    detach_daemon(&home);
    let path = service_file(&home);
    let config = std::fs::read_to_string(&path).expect("the service file");
    std::fs::write(&path, format!("{config}env:\n  TUNED: \"1\"\n")).expect("retune the service");
    revive_daemon(&home);
    wait_for_log(&daemon_log(&home), "\"action\":\"evict\"");

    assert!(
        !process_is_alive(pid),
        "the stale survivor must not outlive its replacement"
    );
    shutdown_daemon(&home);
}

#[test]
fn a_service_whose_program_changed_is_restarted_by_the_new_daemon() {
    let home = verbose_home();
    let script = script_at(home.dir.path(), "while true; do sleep 1; done");
    let cwd = home.root.to_string_lossy();
    let apps = write_apps(
        &home,
        &format!(
            "apps:\n  - name: {SERVICE}\n    script: {}\n    cwd: \"{cwd}\"\n",
            script.display()
        ),
    );
    let started = pm3(&home, &["start", apps.to_str().expect("path")]);
    assert!(started.status.success(), "{}", stdout_of(&started));
    let pid = described_pid(&home, SERVICE);

    detach_daemon(&home);
    script_at(home.dir.path(), "while true; do sleep 2; done");
    revive_daemon(&home);

    assert_ne!(
        described_pid(&home, SERVICE),
        pid,
        "a replaced program must not be reclaimed"
    );
    let log = wait_for_log(&daemon_log(&home), "\"reason\":\"binary\"");
    assert!(log.contains("\"action\":\"respawn\""), "got: {log}");
    shutdown_daemon(&home);
}

#[test]
fn killing_the_daemon_with_its_services_leaves_nothing_running() {
    let home = home();
    let pid = start_sleeper(&home);

    let killed = pm3(&home, &["kill", "--with-services"]);
    assert!(killed.status.success(), "{}", stdout_of(&killed));
    assert!(
        stdout_of(&killed).contains(SERVICE),
        "{}",
        stdout_of(&killed)
    );

    common::wait_until_gone(&home.root.join("pm3.sock"));
    assert!(
        !process_is_alive(pid),
        "--with-services should take the managed service down too"
    );
}

#[test]
fn killing_with_services_reports_a_dump_it_cannot_write() {
    let home = home();
    start_sleeper(&home);
    let dump = home.root.join("dump.yaml");
    std::fs::remove_file(&dump).expect("drop the dump file");
    std::fs::create_dir_all(&dump).expect("block the dump path");
    std::fs::write(dump.join("occupied"), "state").expect("fill the blocked dump path");

    let killed = pm3(&home, &["kill", "--with-services"]);

    assert!(!killed.status.success(), "{}", stdout_of(&killed));
    std::fs::remove_dir_all(&dump).expect("unblock the dump path");
    shutdown_daemon(&home);
}

#[test]
fn killing_the_daemon_alone_leaves_the_service_running() {
    let home = home();
    let pid = start_sleeper(&home);

    let killed = pm3(&home, &["kill"]);
    assert!(killed.status.success(), "{}", stdout_of(&killed));
    assert!(
        stdout_of(&killed).contains("keep running"),
        "{}",
        stdout_of(&killed)
    );

    assert!(
        process_is_alive(pid),
        "a plain kill should leave the service alone"
    );
    shutdown_daemon(&home);
}

const HEALTH_REPLY: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok";
const REQUEST_SINK: usize = 1024;

fn answer_one_probe_then_vanish(socket: &Path) {
    let listener = UnixListener::bind(socket).expect("bind the impostor socket");
    let socket = socket.to_path_buf();
    std::thread::spawn(move || {
        if let Ok((mut stream, _addr)) = listener.accept() {
            let mut sink = vec![0_u8; REQUEST_SINK];
            let read = stream.read(&mut sink).unwrap_or_default();
            sink.truncate(read);
            stream.write_all(HEALTH_REPLY).ok();
        }
        drop(listener);
        std::fs::remove_file(&socket).ok();
    });
}

#[test]
fn killing_a_daemon_that_already_left_is_treated_as_stopped() {
    let home = home();
    answer_one_probe_then_vanish(&home.root.join("pm3.sock"));

    let killed = pm3(&home, &["kill"]);
    assert!(killed.status.success(), "{}", common::stderr_of(&killed));
    assert!(
        stdout_of(&killed).contains("not running"),
        "{}",
        stdout_of(&killed)
    );
}

#[test]
fn killing_a_daemon_that_is_not_running_says_so() {
    let home = home();
    let killed = pm3(&home, &["kill"]);
    assert!(killed.status.success(), "{}", stdout_of(&killed));
    assert!(
        stdout_of(&killed).contains("not running"),
        "{}",
        stdout_of(&killed)
    );
}

#[test]
fn killing_a_daemon_whose_pid_file_vanished_says_what_it_cannot_read() {
    let home = home();
    start_sleeper(&home);
    let pid_file = home.root.join("pm3.pid");
    let recorded = std::fs::read_to_string(&pid_file).expect("the daemon pid file");
    std::fs::remove_file(&pid_file).expect("drop the pid file");

    let killed = pm3(&home, &["kill"]);
    assert!(!killed.status.success(), "{}", stdout_of(&killed));
    assert!(
        common::stderr_of(&killed).contains("cannot read the pm3 daemon pid"),
        "{}",
        common::stderr_of(&killed)
    );

    std::fs::write(&pid_file, recorded).expect("restore the pid file");
    shutdown_daemon(&home);
}

#[test]
fn killing_a_daemon_whose_pid_file_is_bogus_reports_the_refused_signal() {
    let home = home();
    start_sleeper(&home);
    let pid_file = home.root.join("pm3.pid");
    let recorded = std::fs::read_to_string(&pid_file).expect("the daemon pid file");
    std::fs::write(&pid_file, u32::MAX.to_string()).expect("plant a bogus pid");

    let killed = pm3(&home, &["kill"]);
    assert!(!killed.status.success(), "{}", stdout_of(&killed));
    assert!(
        common::stderr_of(&killed).contains("cannot signal pid"),
        "{}",
        common::stderr_of(&killed)
    );

    std::fs::write(&pid_file, recorded).expect("restore the pid file");
    shutdown_daemon(&home);
}

#[test]
fn killing_with_services_needs_a_usable_service_directory() {
    let home = home();
    start_sleeper(&home);
    let cfg_dir = home.root.join("svc");
    std::fs::remove_dir_all(&cfg_dir).expect("clear the service directory");
    std::fs::write(&cfg_dir, "not a directory").expect("occupy the service directory");

    let killed = pm3(&home, &["kill", "--with-services"]);
    assert!(!killed.status.success(), "{}", stdout_of(&killed));
    assert!(
        common::stderr_of(&killed).contains("cannot prepare the pm3 home"),
        "{}",
        common::stderr_of(&killed)
    );

    std::fs::remove_file(&cfg_dir).expect("free the service directory");
    shutdown_daemon(&home);
}

#[test]
fn killing_a_daemon_that_will_not_leave_reports_a_failure() {
    let home = common::impatient_home();
    start_sleeper(&home);
    let pid_file = home.root.join("pm3.pid");
    let recorded = std::fs::read_to_string(&pid_file).expect("the daemon pid file");

    let mut decoy = std::process::Command::new("/bin/sh")
        .args(["-c", "exec sleep 30"])
        .spawn()
        .expect("should spawn a decoy");
    std::fs::write(&pid_file, decoy.id().to_string()).expect("point the pid file at the decoy");

    let killed = pm3(&home, &["kill"]);
    assert!(
        !killed.status.success(),
        "pm3 must not claim success while the daemon is still listening: {}",
        stdout_of(&killed)
    );
    let complaint = common::stderr_of(&killed);
    assert!(
        complaint.contains("is still there after"),
        "got: {complaint}"
    );
    assert!(
        home.root.join("pm3.sock").exists(),
        "the real daemon should still be listening"
    );

    decoy.wait().expect("should reap the decoy");
    std::fs::write(&pid_file, recorded).expect("restore the pid file");
    shutdown_daemon(&home);
}

#[test]
fn deleting_a_selector_that_would_escape_the_apps_path_is_refused() {
    let home = home();
    let deleted = pm3(&home, &["delete", "my app"]);
    assert!(!deleted.status.success(), "{}", stdout_of(&deleted));
    assert!(
        common::stderr_of(&deleted).contains("not allowed"),
        "{}",
        common::stderr_of(&deleted)
    );
}

#[test]
fn deleting_without_a_usable_config_cannot_open_a_session() {
    let deleted = std::process::Command::new(common::PM3)
        .args(["--config", "/nonexistent/pm3.yaml", "delete", "3"])
        .output()
        .expect("pm3 should run");
    assert!(!deleted.status.success(), "{}", stdout_of(&deleted));
    assert!(
        common::stderr_of(&deleted).contains("/nonexistent/pm3.yaml"),
        "{}",
        common::stderr_of(&deleted)
    );
}

#[test]
fn killing_without_a_usable_config_cannot_open_a_session() {
    let killed = std::process::Command::new(common::PM3)
        .args(["--config", "/nonexistent/pm3.yaml", "kill"])
        .output()
        .expect("pm3 should run");
    assert!(!killed.status.success(), "{}", stdout_of(&killed));
    assert!(
        common::stderr_of(&killed).contains("/nonexistent/pm3.yaml"),
        "{}",
        common::stderr_of(&killed)
    );
}
