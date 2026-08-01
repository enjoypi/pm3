#![allow(
    clippy::tests_outside_test_module,
    reason = "integration tests in tests/ are inherently outside #[cfg(test)]"
)]

mod common;

use self::common::{Home, home, pm3, shutdown_daemon, stderr_of, stdout_of, write_apps};

fn half_startable_apps(home: &Home) -> std::path::PathBuf {
    let cwd = home.root.to_string_lossy();
    let unrunnable = home.root.join("not-executable");
    std::fs::write(&unrunnable, "").expect("write a file nobody can execute");
    write_apps(
        home,
        &format!(
            "apps:\n  - name: web\n    script: /bin/sh\n    cwd: \"{cwd}\"\n    args:\n      - \"-c\"\n      - \"sleep 30\"\n  - name: broken\n    script: \"{}\"\n    cwd: \"{cwd}\"\n    depends_on:\n      - web\n",
            unrunnable.display()
        ),
    )
}

fn svc_file(home: &Home, name: &str) -> std::path::PathBuf {
    home.root.join("svc").join(format!("{name}.yaml"))
}

#[test]
fn a_batch_that_only_half_starts_fails_the_command() {
    let home = home();
    let apps = half_startable_apps(&home);
    let started = pm3(&home, &["start", apps.to_str().expect("path")]);
    assert!(
        !started.status.success(),
        "a refused service must fail the command: {}",
        stdout_of(&started)
    );
    assert!(
        stderr_of(&started).contains("cannot start broken"),
        "{}",
        stderr_of(&started)
    );
    shutdown_daemon(&home);
}

#[test]
fn a_batch_that_only_half_starts_keeps_the_service_it_started() {
    let home = home();
    let apps = half_startable_apps(&home);
    pm3(&home, &["start", apps.to_str().expect("path")]);

    assert!(
        svc_file(&home, "web").is_file(),
        "the service that started keeps its service file"
    );
    assert!(
        !svc_file(&home, "broken").exists(),
        "the service that never started is rolled back"
    );

    let listed = pm3(&home, &["list"]);
    assert!(
        stdout_of(&listed).contains("online"),
        "{}",
        stdout_of(&listed)
    );
    shutdown_daemon(&home);
}

#[test]
fn a_service_name_that_escapes_the_service_directory_is_refused() {
    let home = home();
    let cwd = home.root.to_string_lossy();
    let apps = write_apps(
        &home,
        &format!("apps:\n  - name: ../escape\n    script: /bin/sh\n    cwd: \"{cwd}\"\n"),
    );
    let started = pm3(&home, &["start", apps.to_str().expect("path")]);
    assert!(!started.status.success(), "{}", stdout_of(&started));
    assert!(
        !home.root.join("escape.yaml").exists(),
        "nothing may be written outside the service directory"
    );
    shutdown_daemon(&home);
}

#[test]
fn an_apps_file_that_names_the_same_service_twice_is_refused() {
    let home = home();
    let cwd = home.root.to_string_lossy();
    let apps = write_apps(
        &home,
        &format!(
            "apps:\n  - name: web\n    script: /bin/sh\n    cwd: \"{cwd}\"\n  - name: web\n    script: /bin/sh\n    cwd: \"{cwd}\"\n"
        ),
    );
    let started = pm3(&home, &["start", apps.to_str().expect("path")]);
    assert!(
        stderr_of(&started).contains("duplicate app name 'web'"),
        "{}",
        stderr_of(&started)
    );
    shutdown_daemon(&home);
}
