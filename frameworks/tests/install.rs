#![cfg(unix)]
#![allow(
    clippy::tests_outside_test_module,
    reason = "integration tests in tests/ are inherently outside #[cfg(test)]"
)]

mod common;

use std::{
    os::unix::fs::PermissionsExt as _,
    path::{Path, PathBuf},
    process::Output,
    sync::atomic::{AtomicU64, Ordering},
};

#[cfg(target_os = "linux")]
use self::common::impatient_home;
use self::common::{
    PM3, SERVICE_LABEL, described_pid, home_with_timeout, sleeper_apps, stderr_of, stdout_of,
    wait_for_listing,
};

static NEXT_LABEL: AtomicU64 = AtomicU64::new(0);

fn unique_label() -> String {
    format!(
        "pm3-e2e-install-{}-{}",
        std::process::id(),
        NEXT_LABEL.fetch_add(1, Ordering::Relaxed)
    )
}

fn patient_home() -> common::Home {
    home_with_timeout("danger-full-access", true, "info", 30_000)
}

struct InstallFixture {
    home: common::Home,
    label: String,
    fake_home: PathBuf,
    destination: PathBuf,
    backups: PathBuf,
}

#[cfg(target_os = "macos")]
impl Drop for InstallFixture {
    fn drop(&mut self) {
        let uid = std::process::Command::new("id")
            .arg("-u")
            .output()
            .expect("read the uid");
        let uid = String::from_utf8_lossy(&uid.stdout).trim().to_string();
        let _ = std::process::Command::new("launchctl")
            .args(["bootout", &format!("gui/{uid}/{}", self.label)])
            .output();
    }
}

#[cfg(target_os = "macos")]
const MANAGER_NAME: &str = "launchd";
#[cfg(target_os = "linux")]
const MANAGER_NAME: &str = "systemd";

#[cfg(target_os = "macos")]
fn unit_dir(fake_home: &Path) -> PathBuf {
    fake_home.join("Library/LaunchAgents")
}

#[cfg(target_os = "linux")]
fn unit_dir(fake_home: &Path) -> PathBuf {
    fake_home.join(".config/systemd/user")
}

#[cfg(target_os = "macos")]
const UNIT_EXTENSION: &str = "plist";
#[cfg(target_os = "linux")]
const UNIT_EXTENSION: &str = "service";

fn fixture(home: common::Home, manager_body: &str) -> InstallFixture {
    let label = unique_label();
    patch_label(&home, &label);
    let fake_home = home.dir.path().join("fake-home");
    std::fs::create_dir_all(&fake_home).expect("prepare the fake home");
    let destination = home.dir.path().join("dest/pm3");
    let backups = home.dir.path().join("backups");
    let manager = write_manager(&home, &destination, manager_body);
    patch_manager_path(&home, &manager);
    InstallFixture {
        home,
        label,
        fake_home,
        destination,
        backups,
    }
}

fn patch_label(home: &common::Home, label: &str) {
    let yaml = std::fs::read_to_string(&home.config).expect("read the config");
    let patched = yaml.replace(
        &format!("label: \"{SERVICE_LABEL}\""),
        &format!("label: \"{label}\""),
    );
    assert!(
        patched.contains(label),
        "the config carries the unique label"
    );
    std::fs::write(&home.config, patched).expect("patch the config");
}

fn write_manager(home: &common::Home, destination: &Path, body: &str) -> PathBuf {
    let pidfile = home.root.join("pm3.pid");
    let log = home.root.join("pm3.log");
    let script = format!(
        "#!/bin/sh\nDEST=\"{}\"\nCFG=\"{}\"\nPIDFILE=\"{}\"\nLOG=\"{}\"\n{body}\nexit 0\n",
        destination.display(),
        home.config.display(),
        pidfile.display(),
        log.display(),
    );
    let path = home.dir.path().join("fake-systemctl");
    std::fs::write(&path, script).expect("write the fake systemctl");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    path
}

fn patch_manager_path(home: &common::Home, manager: &Path) {
    let yaml = std::fs::read_to_string(&home.config).expect("read the config");
    let patched = yaml.replace(
        "systemctl_path: \"/usr/bin/systemctl\"",
        &format!("systemctl_path: \"{}\"", manager.display()),
    );
    assert!(
        patched.contains(&manager.display().to_string()),
        "the config points at the fake manager"
    );
    std::fs::write(&home.config, patched).expect("patch the config");
}

fn install(fixture: &InstallFixture) -> Output {
    pm3_at(fixture, &["install", PM3])
}

fn pm3_at(fixture: &InstallFixture, args: &[&str]) -> Output {
    let mut command = std::process::Command::new(PM3);
    command
        .arg("--config")
        .arg(&fixture.home.config)
        .args(args)
        .env("HOME", &fixture.fake_home)
        .env("PM3_INSTALL_PATH", &fixture.destination)
        .env("PM3_INSTALL_BACKUPS", &fixture.backups)
        .output()
        .expect("pm3 should run")
}

const SUPERVISING_MANAGER: &str = r#"case "$2" in
  is-active)
    if [ -f "$PIDFILE" ]; then echo active; exit 0; fi
    echo inactive; exit 3
    ;;
  show)
    if [ -f "$0.asked" ]; then
      if [ -f "$PIDFILE" ]; then cat "$PIDFILE"; else echo 0; fi
    else
      touch "$0.asked"
      echo 0
    fi
    ;;
  enable)
    "$DEST" daemon --config "$CFG" </dev/null >>"$LOG" 2>&1 &
    ;;
  disable)
    if [ -f "$PIDFILE" ]; then
      kill "$(cat "$PIDFILE")" 2>/dev/null || true
      rounds=0
      while [ -f "$PIDFILE" ] && [ "$rounds" -lt 40 ]; do
        rounds=$((rounds + 1))
        sleep 0.05
      done
    fi
    ;;
esac
"#;

#[test]
fn a_first_install_lands_the_binary_and_brings_the_daemon_under_supervision() {
    let fixture = fixture(patient_home(), SUPERVISING_MANAGER);

    let installed = install(&fixture);
    assert!(installed.status.success(), "{}", stderr_of(&installed));

    let landed = std::fs::metadata(&fixture.destination).expect("the destination exists");
    let source = std::fs::metadata(PM3).expect("stat pm3");
    assert_eq!(landed.len(), source.len(), "the new binary landed");
    let output = stdout_of(&installed);
    assert!(output.contains("backed up"), "got: {output}");
    assert!(
        output.contains(&format!(
            "service {} ({MANAGER_NAME}) is running",
            fixture.label
        )),
        "got: {output}"
    );
    assert!(
        output.contains("no managed services to reclaim"),
        "got: {output}"
    );
    assert!(
        fixture.backups.read_dir().expect("read backups").count() == 1,
        "one stamp dir"
    );

    let listed = wait_for_listing(&fixture.home, "no apps");
    assert!(listed.contains("no apps"), "got: {listed}");
}

#[test]
fn an_upgrade_adopts_the_running_service_and_backs_up_the_previous_install() {
    let fixture = fixture(patient_home(), SUPERVISING_MANAGER);
    std::fs::create_dir_all(fixture.destination.parent().expect("dest dir")).expect("mkdir dest");
    std::fs::write(&fixture.destination, "#!/bin/sh\necho 'pm3 1.7.1'\n")
        .expect("write the old binary");
    std::fs::set_permissions(&fixture.destination, std::fs::Permissions::from_mode(0o755))
        .expect("chmod the old binary");
    std::fs::write(fixture.home.root.join("config.yaml"), "old config").expect("seed config");
    let unit_dir = unit_dir(&fixture.fake_home);
    std::fs::create_dir_all(&unit_dir).expect("mkdir unit dir");
    std::fs::write(
        unit_dir.join(format!("{}.{UNIT_EXTENSION}", fixture.label)),
        "old unit",
    )
    .expect("seed unit");

    let apps = sleeper_apps(&fixture.home, "web");
    let started = pm3_at(&fixture, &["start", apps.to_str().expect("utf8")]);
    assert!(started.status.success(), "{}", stdout_of(&started));
    let before_pid = described_pid(&fixture.home, "web");

    let upgraded = install(&fixture);
    assert!(upgraded.status.success(), "{}", stderr_of(&upgraded));
    let output = stdout_of(&upgraded);
    assert!(output.contains("adopted 1: web"), "got: {output}");
    assert_eq!(described_pid(&fixture.home, "web"), before_pid);

    let stamp = fixture
        .backups
        .read_dir()
        .expect("read backups")
        .next()
        .expect("one stamp dir")
        .expect("dir entry")
        .path();
    assert_eq!(
        stamp,
        fixture.backups.join("1.7.1"),
        "named by the old version"
    );
    assert_eq!(
        std::fs::read_to_string(stamp.join("pm3")).expect("binary backup"),
        "#!/bin/sh\necho 'pm3 1.7.1'\n"
    );
    assert_eq!(
        std::fs::read_to_string(stamp.join("config.yaml")).expect("config backup"),
        "old config"
    );
    assert!(
        stamp
            .join(format!("{}.{UNIT_EXTENSION}", fixture.label))
            .is_file(),
        "unit backup"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn a_takeover_that_never_happens_fails_and_points_at_the_backup() {
    let silent_manager = r#"case "$2" in
  is-active) echo inactive; exit 3 ;;
  show) echo 0 ;;
esac
"#;
    let fixture = fixture(impatient_home(), silent_manager);

    let failed = install(&fixture);
    assert!(!failed.status.success(), "{}", stdout_of(&failed));
    let message = stderr_of(&failed);
    assert!(message.contains("did not come under"), "got: {message}");
    assert!(message.contains("restore the binary"), "got: {message}");
    assert!(
        std::fs::metadata(&fixture.destination)
            .expect("the new binary landed")
            .len()
            == std::fs::metadata(PM3).expect("stat pm3").len(),
        "the binary was still swapped"
    );
}
