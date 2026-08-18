#![cfg(windows)]
#![allow(
    clippy::tests_outside_test_module,
    reason = "integration tests in tests/ are inherently outside #[cfg(test)]"
)]

mod common;

use std::process::Output;

use self::common::{Home, PM3, SERVICE_LABEL, home, stdout_of};

fn pm3_as_user(home: &Home, args: &[&str]) -> Output {
    std::process::Command::new(PM3)
        .arg("--config")
        .arg(&home.config)
        .args(args)
        .env("HOME", home.dir.path())
        .env("USERPROFILE", home.dir.path())
        .output()
        .expect("pm3 should run")
}

#[test]
fn a_dry_run_install_prints_the_task_xml_and_the_wrapper() {
    let home = home();
    let planned = pm3_as_user(&home, &["startup", "--dry-run"]);
    assert!(planned.status.success(), "{}", stdout_of(&planned));
    let printed = stdout_of(&planned);
    assert!(printed.contains("<LogonTrigger>"), "{printed}");
    assert!(
        printed.contains("<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>"),
        "{printed}"
    );
    assert!(printed.contains("-daemon.cmd"), "{printed}");
    assert!(printed.contains("exit /b 1"), "{printed}");
    assert!(printed.contains("/Create /TN"), "{printed}");
}

#[test]
fn a_service_install_registers_the_task_and_uninstall_removes_it() {
    let home = home();
    let installed = pm3_as_user(&home, &["startup"]);
    assert!(installed.status.success(), "{}", stdout_of(&installed));

    let registered = std::process::Command::new("schtasks")
        .args(["/Query", "/TN", SERVICE_LABEL])
        .output()
        .expect("schtasks should run");
    assert!(
        registered.status.success(),
        "the task should be registered: {}",
        String::from_utf8_lossy(&registered.stderr)
    );

    let running = pm3_as_user(&home, &["startup", "--status"]);
    assert!(
        stdout_of(&running).contains("running"),
        "{}",
        stdout_of(&running)
    );

    let uninstalled = pm3_as_user(&home, &["unstartup"]);
    assert!(uninstalled.status.success(), "{}", stdout_of(&uninstalled));
    let gone = std::process::Command::new("schtasks")
        .args(["/Query", "/TN", SERVICE_LABEL])
        .output()
        .expect("schtasks should run");
    assert!(!gone.status.success(), "the task should be deleted");
    let service_dir = home.dir.path().join(".pm3/service");
    assert!(
        !service_dir.join(format!("{SERVICE_LABEL}.xml")).exists(),
        "the task xml should be removed"
    );
    assert!(
        !service_dir
            .join(format!("{SERVICE_LABEL}-daemon.cmd"))
            .exists(),
        "the wrapper should be removed"
    );
}
