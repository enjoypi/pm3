#![cfg(unix)]
#![allow(
    clippy::tests_outside_test_module,
    reason = "integration tests in tests/ are inherently outside #[cfg(test)]"
)]

mod common;

use std::path::{Path, PathBuf};

use self::common::{PM3, stdout_of};

struct SplitHome {
    dir: tempfile::TempDir,
    config: PathBuf,
}

impl SplitHome {
    fn config_root(&self) -> PathBuf {
        self.dir.path().join("config")
    }

    fn state_root(&self) -> PathBuf {
        self.dir.path().join("state")
    }

    fn runtime_root(&self) -> PathBuf {
        self.dir.path().join("run")
    }

    fn data_root(&self) -> PathBuf {
        self.dir.path().join("data")
    }
}

fn split_home() -> SplitHome {
    let dir = tempfile::tempdir().expect("temp dir");
    let config_root = dir.path().join("config");
    std::fs::create_dir_all(&config_root).expect("prepare the config root");
    let config = config_root.join("config.yaml");
    std::fs::write(&config, split_config_yaml()).expect("write the pm3 config");
    SplitHome { dir, config }
}

fn split_config_yaml() -> String {
    let body = common::config_yaml(
        "",
        "workspace-write",
        "minimal",
        false,
        "debug",
        5000,
        &common::HomeTunables::default(),
    );
    body.replace("  cfg_dir: \"/service\"\n", "  cfg_dir: \"\"\n")
}

fn pm3_split(home: &SplitHome, args: &[&str]) -> std::process::Output {
    std::process::Command::new(PM3)
        .arg("--config")
        .arg(&home.config)
        .args(args)
        .env("PM3_CONFIG_DIR", home.config_root())
        .env("PM3_STATE_DIR", home.state_root())
        .env("PM3_RUNTIME_DIR", home.runtime_root())
        .env("PM3_DATA_DIR", home.data_root())
        .env_remove("PM3_HOME")
        .output()
        .expect("pm3 should run")
}

fn owner_only(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::metadata(path)
        .expect("the directory should exist")
        .permissions()
        .mode()
        & 0o777
}

#[test]
fn a_daemon_started_with_the_split_layout_serves_the_cli() {
    let home = split_home();

    let listed = pm3_split(&home, &["list"]);
    assert!(
        listed.status.success(),
        "the cli and the daemon must agree on the socket path: {}",
        String::from_utf8_lossy(&listed.stderr)
    );
    assert!(
        stdout_of(&listed).contains("no apps"),
        "got: {}",
        stdout_of(&listed)
    );

    let socket = home.runtime_root().join("pm3.sock");
    assert!(
        socket.exists(),
        "the socket belongs to the runtime root, got {}",
        socket.to_string_lossy()
    );
    assert!(
        home.state_root().join("dump.yaml").exists(),
        "the dump belongs to the state root"
    );
    assert!(
        home.state_root().join("logs").is_dir(),
        "the logs belong to the state root"
    );

    let _ = pm3_split(&home, &["shutdown"]);
}

#[test]
fn the_split_layout_keeps_every_root_to_its_owner() {
    let home = split_home();

    let listed = pm3_split(&home, &["list"]);
    assert!(listed.status.success(), "{}", stdout_of(&listed));

    for root in [
        home.config_root(),
        home.state_root(),
        home.runtime_root(),
        home.data_root(),
    ] {
        assert_eq!(
            owner_only(&root),
            0o700,
            "every pm3 root stays private: {}",
            root.to_string_lossy()
        );
    }

    let _ = pm3_split(&home, &["shutdown"]);
}

#[test]
fn a_confined_app_writes_its_cwd_while_the_state_root_stays_hidden() {
    let home = split_home();
    let cfg_dir = home.config_root();
    let apps = cfg_dir.join("apps.yaml");
    let dump = home.state_root().join("dump.yaml");
    std::fs::write(
        &apps,
        format!(
            "apps:\n  - name: probe\n    script: /bin/sh\n    autorestart: false\n    args:\n      - \"-c\"\n      - \"echo mine > own.txt && (cat '{}' > leaked.txt 2>/dev/null && echo leaked || echo blocked) && echo done\"\n",
            dump.to_string_lossy()
        ),
    )
    .expect("write the apps file");

    let started = pm3_split(&home, &["start", apps.to_str().expect("path")]);
    assert!(started.status.success(), "{}", stdout_of(&started));
    let log = home.state_root().join("logs").join("probe-out.log");
    wait_for_line(&log, "done");

    let cwd = home.state_root().join("apps").join("probe");
    assert!(
        cwd.join("own.txt").exists(),
        "the working directory must stay writable under a hidden state root"
    );
    let seen = std::fs::read_to_string(&log).expect("read the probe log");
    assert!(
        seen.contains("blocked"),
        "the dump belongs to pm3, not to the service, got: {seen}"
    );

    let _ = pm3_split(&home, &["shutdown", "--with-services"]);
}

fn wait_for_line(log: &Path, needle: &str) {
    for _ in 0..200 {
        if std::fs::read_to_string(log).is_ok_and(|text| text.contains(needle)) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!(
        "the probe never logged '{needle}': {}",
        std::fs::read_to_string(log).unwrap_or_default()
    );
}

#[test]
fn a_runtime_root_that_overflows_the_socket_limit_is_refused() {
    let home = split_home();
    let deep = home.dir.path().join("d".repeat(120));

    let refused = std::process::Command::new(PM3)
        .arg("--config")
        .arg(&home.config)
        .arg("list")
        .env("PM3_CONFIG_DIR", home.config_root())
        .env("PM3_STATE_DIR", home.state_root())
        .env("PM3_RUNTIME_DIR", &deep)
        .env("PM3_DATA_DIR", home.data_root())
        .env_remove("PM3_HOME")
        .output()
        .expect("pm3 should run");

    assert!(
        !refused.status.success(),
        "a socket pm3 cannot bind must fail"
    );
    let complaint = String::from_utf8_lossy(&refused.stderr);
    assert!(
        complaint.contains("cannot accept the socket path"),
        "the refusal must name the limit, got: {complaint}"
    );
}

fn refuse_with_deep_runtime(home: &SplitHome, args: &[&str]) -> String {
    let deep = home.dir.path().join("d".repeat(120));
    let refused = std::process::Command::new(PM3)
        .arg("--config")
        .arg(&home.config)
        .args(args)
        .env("PM3_CONFIG_DIR", home.config_root())
        .env("PM3_STATE_DIR", home.state_root())
        .env("PM3_RUNTIME_DIR", &deep)
        .env("PM3_DATA_DIR", home.data_root())
        .env_remove("PM3_HOME")
        .output()
        .expect("pm3 should run");
    assert!(
        !refused.status.success(),
        "a socket pm3 cannot bind must fail: {}",
        stdout_of(&refused)
    );
    String::from_utf8_lossy(&refused.stderr).into_owned()
}

#[test]
fn a_daemon_refuses_a_runtime_root_that_overflows_the_socket_limit() {
    let home = split_home();
    let complaint = refuse_with_deep_runtime(&home, &["daemon"]);
    assert!(
        complaint.contains("cannot accept the socket path"),
        "the daemon must refuse before binding, got: {complaint}"
    );
}

#[test]
fn startup_refuses_a_runtime_root_that_overflows_the_socket_limit() {
    let home = split_home();
    let complaint = refuse_with_deep_runtime(&home, &["startup", "--dry-run"]);
    assert!(
        complaint.contains("cannot accept the socket path"),
        "rendering a unit must refuse the same way, got: {complaint}"
    );
}
