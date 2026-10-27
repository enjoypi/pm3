#![cfg(unix)]
#![allow(
    clippy::tests_outside_test_module,
    reason = "integration tests in tests/ are inherently outside #[cfg(test)]"
)]

mod common;

use std::{os::unix::fs::PermissionsExt as _, path::PathBuf};

use self::common::{
    Home, NOTIFIER, PM3, home_with_memory_poll, shell_app, shutdown_daemon, wait_for_file,
    write_apps,
};

fn install_recorder(home: &Home) -> PathBuf {
    let notified = home.root.join("notified");
    let stand_in = home.root.join(NOTIFIER);
    let body = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}.tmp'\nmv '{0}.tmp' '{0}'\n",
        notified.display()
    );
    std::fs::write(&stand_in, body).expect("write the notifier stand-in");
    std::fs::set_permissions(&stand_in, std::fs::Permissions::from_mode(0o755))
        .expect("make the notifier executable");
    notified
}

#[test]
fn a_failed_one_shot_raises_a_desktop_notification() {
    let home = common::home();
    let notified = install_recorder(&home);
    let quitter = shell_app(&home, "quitter", "exit 7");
    let apps = write_apps(&home, &format!("apps:\n{quitter}    autorestart: false\n"));
    common::start_ok(&home, &apps);

    wait_for_file(&notified);
    let text = std::fs::read_to_string(&notified).expect("read the notification");
    assert!(text.contains("pm3: quitter"), "got: {text}");
    assert!(text.contains("退出码 7"), "got: {text}");
    shutdown_daemon(&home);
}

#[test]
fn a_memory_breach_raises_a_desktop_notification() {
    let home = home_with_memory_poll(200);
    let notified = install_recorder(&home);
    let cwd = home.root.to_string_lossy();
    let apps = write_apps(
        &home,
        &format!(
            "apps:\n  - name: hog\n    script: {PM3}\n    cwd: '{cwd}'\n    max_memory: \"1K\"\n    args:\n      - \"__sleep\"\n      - \"30000\"\n"
        ),
    );
    common::start_ok(&home, &apps);

    wait_for_file(&notified);
    let text = std::fs::read_to_string(&notified).expect("read the notification");
    assert!(text.contains("内存超限"), "got: {text}");
    shutdown_daemon(&home);
}
