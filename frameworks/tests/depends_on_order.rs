#![allow(
    clippy::tests_outside_test_module,
    reason = "integration tests in tests/ are inherently outside #[cfg(test)]"
)]

mod common;

use self::common::{
    Home, SHELL, SHELL_FLAG, SLEEPER, daemon_log, home, pm3, record_then_sleep, shutdown_daemon,
    stderr_of, stdout_of, wait_for_log, write_apps,
};

const ORDER_FILE: &str = "order.txt";

fn ordered_apps(home: &Home) -> std::path::PathBuf {
    let cwd = home.root.to_string_lossy();
    let web = record_then_sleep("web", ORDER_FILE);
    let db = record_then_sleep("db", ORDER_FILE);
    write_apps(
        home,
        &format!(
            "apps:\n  - name: web\n    script: '{SHELL}'\n    cwd: '{cwd}'\n    depends_on:\n      - db\n    args:\n      - \"{SHELL_FLAG}\"\n      - '{web}'\n  - name: db\n    script: '{SHELL}'\n    cwd: '{cwd}'\n    args:\n      - \"{SHELL_FLAG}\"\n      - '{db}'\n"
        ),
    )
}

fn cyclic_apps(home: &Home) -> std::path::PathBuf {
    let cwd = home.root.to_string_lossy();
    write_apps(
        home,
        &format!(
            "apps:\n  - name: web\n    script: '{SHELL}'\n    cwd: '{cwd}'\n    depends_on:\n      - db\n    args:\n      - \"{SHELL_FLAG}\"\n      - '{SLEEPER}'\n  - name: db\n    script: '{SHELL}'\n    cwd: '{cwd}'\n    depends_on:\n      - web\n    args:\n      - \"{SHELL_FLAG}\"\n      - '{SLEEPER}'\n"
        ),
    )
}

#[test]
fn a_dependency_is_spawned_before_the_app_that_needs_it() {
    let home = home();
    let apps = ordered_apps(&home);
    let started = pm3(&home, &["start", apps.to_str().expect("path")]);
    assert!(started.status.success(), "{}", stdout_of(&started));

    let journal = wait_for_log(&daemon_log(&home), "\"app\":\"web\"");
    let spawn_order: Vec<&str> = journal
        .lines()
        .filter(|line| line.contains("\"action\":\"spawn\""))
        .filter_map(|line| {
            ["db", "web"]
                .into_iter()
                .find(|app| line.contains(&format!("\"app\":\"{app}\"")))
        })
        .collect();
    assert_eq!(spawn_order, vec!["db", "web"]);
    shutdown_daemon(&home);
}

#[test]
fn both_apps_record_themselves() {
    let home = home();
    let apps = ordered_apps(&home);
    pm3(&home, &["start", apps.to_str().expect("path")]);
    let recorded = wait_for_log(&home.root.join(ORDER_FILE), "web");
    assert_eq!(recorded.lines().count(), 2, "got: {recorded}");
    shutdown_daemon(&home);
}

#[test]
fn a_dependency_cycle_is_refused_without_leaving_service_files() {
    let home = home();
    let apps = cyclic_apps(&home);
    let started = pm3(&home, &["start", apps.to_str().expect("path")]);
    assert!(!started.status.success(), "a cycle must fail the command");
    assert!(
        stderr_of(&started).contains("dependency cycle"),
        "got: {}",
        stderr_of(&started)
    );
    let leftovers: Vec<String> = std::fs::read_dir(home.root.join("service"))
        .expect("the service directory")
        .map(|entry| {
            entry
                .expect("a service directory entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert!(
        leftovers.is_empty(),
        "a refused start must roll its service files back, found: {leftovers:?}"
    );
    shutdown_daemon(&home);
}

#[test]
fn an_unknown_dependency_is_refused() {
    let home = home();
    let cwd = home.root.to_string_lossy();
    let apps = write_apps(
        &home,
        &format!(
            "apps:\n  - name: web\n    script: '{SHELL}'\n    cwd: '{cwd}'\n    depends_on:\n      - ghost\n    args:\n      - \"{SHELL_FLAG}\"\n      - '{SLEEPER}'\n"
        ),
    );
    let started = pm3(&home, &["start", apps.to_str().expect("path")]);
    assert!(
        !started.status.success(),
        "an unknown dependency must fail the command"
    );
    shutdown_daemon(&home);
}
