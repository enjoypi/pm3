#![allow(
    clippy::tests_outside_test_module,
    reason = "integration tests in tests/ are inherently outside #[cfg(test)]"
)]

mod common;

use self::common::{PM3, home, pm3, shutdown_daemon, stderr_of, stdout_of};

const NAME: &str = "sleeper";

fn svc_file(home: &self::common::Home) -> std::path::PathBuf {
    home.root.join("svc").join(format!("{NAME}.yaml"))
}

fn start_inline(home: &self::common::Home, extra: &[&str]) -> std::process::Output {
    let mut args = vec!["start", "--name", NAME];
    args.extend_from_slice(extra);
    args.extend_from_slice(&[PM3, "__sleep", "30000"]);
    pm3(home, &args)
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

    assert!(svc_file(&home).is_file(), "the service file should exist");
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
    let written = std::fs::read_to_string(svc_file(&home)).expect("read the config file");
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
    std::fs::write(svc_file(&home), "apps: []\n").expect("edit the service file");

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
fn deleting_a_service_removes_its_file() {
    let home = home();
    assert!(start_inline(&home, &[]).status.success(), "first start");
    let deleted = pm3(&home, &["delete", NAME]);
    assert!(deleted.status.success(), "{}", stderr_of(&deleted));
    assert!(
        !svc_file(&home).exists(),
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
