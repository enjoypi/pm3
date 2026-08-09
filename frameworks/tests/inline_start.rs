#![cfg(unix)]
#![allow(
    clippy::tests_outside_test_module,
    reason = "integration tests in tests/ are inherently outside #[cfg(test)]"
)]

mod common;

use self::common::{
    PM3, home, pm3, pm3_with_stdin, shutdown_daemon, stderr_of, stdout_of, wait_for_listing,
};

const NAME: &str = "sleeper";

fn service_file_at(home: &self::common::Home) -> std::path::PathBuf {
    home.root.join("service").join(format!("{NAME}.yaml"))
}

fn start_inline(home: &self::common::Home, extra: &[&str]) -> std::process::Output {
    let mut args = vec!["start", "--name", NAME];
    args.extend_from_slice(extra);
    args.extend_from_slice(&[PM3, "__sleep", "30000"]);
    pm3(home, &args)
}

fn start_changed_with_answer(home: &self::common::Home, answer: &str) -> std::process::Output {
    let args = [
        "start",
        "--name",
        NAME,
        "--force",
        "--writable-dir",
        "/srv/fresh",
        PM3,
        "__sleep",
        "30000",
    ];
    pm3_with_stdin(home, &args, answer)
}

#[test]
fn an_inline_program_becomes_a_managed_service() {
    let home = home();
    let started = start_inline(&home, &[]);
    assert!(started.status.success(), "{}", stderr_of(&started));
    assert!(
        stdout_of(&started).contains("started sleeper"),
        "{}",
        stdout_of(&started)
    );

    let listed = pm3(&home, &["list"]);
    assert!(stdout_of(&listed).contains(NAME), "{}", stdout_of(&listed));
    assert!(
        stdout_of(&listed).contains("online"),
        "{}",
        stdout_of(&listed)
    );

    assert!(
        service_file_at(&home).is_file(),
        "the service file should exist"
    );
    assert!(
        home.root.join(NAME).is_dir(),
        "the working directory should exist"
    );

    shutdown_daemon(&home);
}

#[test]
fn the_config_file_carries_no_machine_specific_paths() {
    let home = home();
    let started = start_inline(&home, &[]);
    assert!(started.status.success(), "{}", stderr_of(&started));
    let written = std::fs::read_to_string(service_file_at(&home)).expect("read the config file");
    assert!(written.contains("__sleep"), "{written}");
    assert!(
        !written.contains("cwd:"),
        "the daemon derives the cwd: {written}"
    );
    let host_home = std::env::var("HOME").expect("tests always run with HOME");
    assert!(
        !written.contains(&host_home),
        "the home must be folded away: {written}"
    );
    assert!(
        home.root.join(NAME).is_dir(),
        "the daemon should create the working directory"
    );
    shutdown_daemon(&home);
}

#[test]
fn a_changed_service_file_blocks_a_restart_until_forced() {
    let home = home();
    assert!(start_inline(&home, &[]).status.success(), "first start");
    std::fs::write(service_file_at(&home), "apps: []\n").expect("edit the service file");

    let refused = start_inline(&home, &[]);
    assert!(!refused.status.success(), "a changed file needs --force");
    assert!(
        stderr_of(&refused).contains("without --force"),
        "{}",
        stderr_of(&refused)
    );
    assert!(
        stderr_of(&refused).contains("-apps: []"),
        "{}",
        stderr_of(&refused)
    );

    let forced = start_inline(&home, &["--force"]);
    assert!(forced.status.success(), "{}", stderr_of(&forced));
    shutdown_daemon(&home);
}

#[test]
fn answering_yes_restarts_the_running_service_after_a_config_change() {
    let home = home();
    assert!(start_inline(&home, &[]).status.success(), "first start");

    let answered = start_changed_with_answer(&home, "y\n");
    assert!(answered.status.success(), "{}", stderr_of(&answered));
    let shown = stdout_of(&answered);
    assert!(shown.contains("is already running"), "{shown}");
    assert!(shown.contains("restart to apply? [y/N]"), "{shown}");
    assert!(shown.contains("restarted sleeper"), "{shown}");

    wait_for_listing(&home, "online");
    shutdown_daemon(&home);
}

#[test]
fn answering_no_keeps_the_running_service_on_the_old_config() {
    let home = home();
    assert!(start_inline(&home, &[]).status.success(), "first start");

    let answered = start_changed_with_answer(&home, "n\n");
    assert!(answered.status.success(), "{}", stderr_of(&answered));
    let shown = stdout_of(&answered);
    assert!(
        shown.contains("keeps running with the previous config"),
        "{shown}"
    );
    assert!(!shown.contains("restarted"), "{shown}");

    let listed = pm3(&home, &["list"]);
    assert!(
        stdout_of(&listed).contains("online"),
        "{}",
        stdout_of(&listed)
    );
    shutdown_daemon(&home);
}

#[test]
fn a_closed_stdin_keeps_the_running_service_on_the_old_config() {
    let home = home();
    assert!(start_inline(&home, &[]).status.success(), "first start");

    let forced = start_inline(&home, &["--force", "--writable-dir", "/srv/fresh"]);
    assert!(forced.status.success(), "{}", stderr_of(&forced));
    let shown = stdout_of(&forced);
    assert!(
        shown.contains("keeps running with the previous config"),
        "{shown}"
    );
    assert!(!shown.contains("restarted"), "{shown}");
    shutdown_daemon(&home);
}

#[test]
fn an_identical_restart_neither_prompts_nor_restarts() {
    let home = home();
    assert!(start_inline(&home, &[]).status.success(), "first start");

    let again = start_inline(&home, &[]);
    assert!(again.status.success(), "{}", stderr_of(&again));
    let shown = stdout_of(&again);
    assert!(shown.contains("is already running"), "{shown}");
    assert!(!shown.contains("restart to apply"), "{shown}");
    shutdown_daemon(&home);
}

#[test]
fn a_changed_config_for_a_stopped_service_starts_it_without_a_prompt() {
    let home = home();
    assert!(start_inline(&home, &[]).status.success(), "first start");
    assert!(pm3(&home, &["stop", NAME]).status.success(), "stop");

    let forced = start_inline(&home, &["--force", "--writable-dir", "/srv/fresh"]);
    assert!(forced.status.success(), "{}", stderr_of(&forced));
    let shown = stdout_of(&forced);
    assert!(shown.contains("started sleeper"), "{shown}");
    assert!(!shown.contains("restart to apply"), "{shown}");
    shutdown_daemon(&home);
}

#[test]
fn deleting_a_service_removes_its_file() {
    let home = home();
    assert!(start_inline(&home, &[]).status.success(), "first start");
    let deleted = pm3(&home, &["delete", NAME]);
    assert!(deleted.status.success(), "{}", stderr_of(&deleted));
    assert!(
        !service_file_at(&home).exists(),
        "the service file should be removed"
    );
    shutdown_daemon(&home);
}

#[test]
fn a_program_that_is_not_on_the_search_path_is_refused() {
    let home = home();
    let refused = pm3(&home, &["start", "--name", NAME, "pm3-not-a-real-program"]);
    assert!(!refused.status.success(), "an unknown program should fail");
    assert!(
        stderr_of(&refused).contains("on PATH"),
        "{}",
        stderr_of(&refused)
    );
    shutdown_daemon(&home);
}

#[test]
fn starting_without_a_target_explains_the_usage() {
    let home = home();
    let refused = pm3(&home, &["start"]);
    assert!(!refused.status.success(), "start needs a target");
    assert!(
        stderr_of(&refused).contains("exactly one apps file"),
        "{}",
        stderr_of(&refused)
    );
}

#[test]
fn naming_a_service_without_a_program_explains_the_usage() {
    let home = home();
    let refused = pm3(&home, &["start", "--name", "probe"]);
    assert!(!refused.status.success(), "--name needs a program");
    assert!(
        stderr_of(&refused).contains("needs a program"),
        "{}",
        stderr_of(&refused)
    );
}

#[test]
fn starting_an_apps_file_that_is_not_there_is_refused() {
    let home = home();
    let refused = pm3(&home, &["start", "/nonexistent/pm3-apps.yaml"]);
    assert!(!refused.status.success(), "a missing apps file should fail");
    assert!(
        stderr_of(&refused).contains("cannot resolve the apps file"),
        "{}",
        stderr_of(&refused)
    );
}

#[test]
fn starting_an_apps_file_that_is_a_directory_is_refused() {
    let home = home();
    let blocked = home.dir.path().join("apps-as-a-directory");
    std::fs::create_dir(&blocked).expect("occupy the apps file path");
    let refused = pm3(&home, &["start", &blocked.to_string_lossy()]);
    assert!(!refused.status.success(), "a directory is not an apps file");
    assert!(
        stderr_of(&refused).contains("cannot read apps file"),
        "{}",
        stderr_of(&refused)
    );
}

#[test]
fn deleting_an_unknown_service_is_refused() {
    let home = home();
    let refused = pm3(&home, &["delete", "ghost"]);
    assert!(!refused.status.success(), "an unknown app should fail");
    shutdown_daemon(&home);
}
