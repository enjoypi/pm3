use std::path::Path;

use super::*;

const TIMEOUT_MS: u64 = 5000;
use crate::{
    UnitKind, UnitProgramSet, UnitSpec,
    unit_specs::{fake_program, program_set, spec_for},
};

const TRUE_PROGRAM: &str = "/usr/bin/true";
const FALSE_PROGRAM: &str = "/usr/bin/false";
const CONFIG_BODY: &str = "pm3:\n  home: \"~/.pm3\"\n";

fn installed_spec(home: &Path, kind: UnitKind) -> UnitSpec {
    let spec = spec_for(kind, home);
    std::fs::create_dir_all(&spec.unit_dir).expect("prepare the unit directory");
    std::fs::write(spec.unit_path(), "unit body").expect("install a unit file");
    spec
}

#[tokio::test]
async fn a_dry_run_install_prints_the_plan_and_leaves_the_disk_alone() {
    let dir = tempfile::tempdir().expect("temp dir");
    let spec = spec_for(UnitKind::Launchd, dir.path());
    let report = install_unit(
        &spec,
        &program_set(FALSE_PROGRAM),
        CONFIG_BODY,
        true,
        TIMEOUT_MS,
    )
    .await
    .expect("a dry run should never fail");
    assert!(report.contains("<key>RunAtLoad</key>"), "got: {report}");
    assert!(
        report.contains("run /usr/bin/false load -w"),
        "got: {report}"
    );
    assert!(!spec.unit_path().exists(), "a dry run must not write");
}

#[tokio::test]
async fn an_install_writes_the_unit_and_activates_it() {
    let dir = tempfile::tempdir().expect("temp dir");
    let spec = spec_for(UnitKind::Systemd, dir.path());
    let report = install_unit(
        &spec,
        &program_set(TRUE_PROGRAM),
        CONFIG_BODY,
        false,
        TIMEOUT_MS,
    )
    .await
    .expect("the install should succeed");
    assert!(report.contains("installed pm3-test"), "got: {report}");
    assert!(
        std::fs::read_to_string(spec.unit_path())
            .expect("read the unit")
            .contains("WantedBy=default.target")
    );
}

#[tokio::test]
async fn a_dry_run_systemd_install_marks_linger_as_optional() {
    let dir = tempfile::tempdir().expect("temp dir");
    let spec = spec_for(UnitKind::Systemd, dir.path());
    let report = install_unit(
        &spec,
        &program_set(FALSE_PROGRAM),
        CONFIG_BODY,
        true,
        TIMEOUT_MS,
    )
    .await
    .expect("a dry run should never fail");
    assert!(
        report.contains("try /usr/bin/false enable-linger"),
        "got: {report}"
    );
}

#[tokio::test]
async fn an_install_survives_a_refused_linger() {
    let dir = tempfile::tempdir().expect("temp dir");
    let spec = spec_for(UnitKind::Systemd, dir.path());
    let programs = UnitProgramSet {
        launchctl: TRUE_PROGRAM.to_string(),
        systemctl: TRUE_PROGRAM.to_string(),
        loginctl: FALSE_PROGRAM.to_string(),
        runtime_dir: None,
        uid: Some(4242),
    };
    let report = install_unit(&spec, &programs, CONFIG_BODY, false, TIMEOUT_MS)
        .await
        .expect("a refused linger must not fail the install");
    assert!(
        report.contains("skipped: cannot complete '/usr/bin/false'"),
        "got: {report}"
    );
}

#[tokio::test]
async fn an_install_never_asks_a_lingering_user_for_permission() {
    let dir = tempfile::tempdir().expect("temp dir");
    let spec = spec_for(UnitKind::Systemd, dir.path());
    let loginctl = fake_program(
        dir.path(),
        "loginctl",
        "case \"$1\" in show-user) echo yes;; *) exit 1;; esac",
    );
    let programs = UnitProgramSet {
        launchctl: TRUE_PROGRAM.to_string(),
        systemctl: TRUE_PROGRAM.to_string(),
        loginctl,
        runtime_dir: None,
        uid: Some(4242),
    };

    let report = install_unit(&spec, &programs, CONFIG_BODY, false, TIMEOUT_MS)
        .await
        .expect("the install should succeed");

    assert!(
        !report.contains("skipped"),
        "an operator without sudo must see a clean install: {report}"
    );
}

#[tokio::test]
async fn an_install_reports_a_manager_refusal() {
    let dir = tempfile::tempdir().expect("temp dir");
    let spec = spec_for(UnitKind::Launchd, dir.path());
    let err = install_unit(
        &spec,
        &program_set(FALSE_PROGRAM),
        CONFIG_BODY,
        false,
        TIMEOUT_MS,
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(err.contains("exited with status 1"), "got: {err}");
}

#[tokio::test]
async fn an_install_settles_the_config_into_the_pm3_home() {
    let dir = tempfile::tempdir().expect("temp dir");
    let spec = spec_for(UnitKind::Systemd, dir.path());
    install_unit(
        &spec,
        &program_set(TRUE_PROGRAM),
        CONFIG_BODY,
        false,
        TIMEOUT_MS,
    )
    .await
    .expect("the install should succeed");
    assert_eq!(
        std::fs::read_to_string(&spec.config_path).expect("read the settled config"),
        CONFIG_BODY
    );
}

#[tokio::test]
async fn a_dry_run_uninstall_prints_the_plan() {
    let dir = tempfile::tempdir().expect("temp dir");
    let spec = spec_for(UnitKind::Launchd, dir.path());
    let report = uninstall_unit(&spec, &program_set(FALSE_PROGRAM), true, TIMEOUT_MS)
        .await
        .expect("a dry run should never fail");
    assert!(
        report.contains("try /usr/bin/false unload -w"),
        "got: {report}"
    );
    assert!(report.contains("remove "), "got: {report}");
}

#[tokio::test]
async fn uninstalling_what_was_never_installed_is_a_noop() {
    let dir = tempfile::tempdir().expect("temp dir");
    let spec = spec_for(UnitKind::Launchd, dir.path());
    let report = uninstall_unit(&spec, &program_set(FALSE_PROGRAM), false, TIMEOUT_MS)
        .await
        .expect("a missing service is not an error");
    assert_eq!(report, NOTHING_INSTALLED);
}

#[tokio::test]
async fn an_uninstall_deactivates_the_service_and_removes_the_unit() {
    let dir = tempfile::tempdir().expect("temp dir");
    let spec = installed_spec(dir.path(), UnitKind::Systemd);
    let report = uninstall_unit(&spec, &program_set(TRUE_PROGRAM), false, TIMEOUT_MS)
        .await
        .expect("the uninstall should succeed");
    assert_eq!(report, "uninstalled pm3-test");
    assert!(!spec.unit_path().exists(), "the unit file should be gone");
}

#[tokio::test]
async fn an_uninstall_that_cannot_remove_the_unit_is_reported() {
    let dir = tempfile::tempdir().expect("temp dir");
    let spec = installed_spec(dir.path(), UnitKind::Launchd);
    let vanishing = fake_program(dir.path(), "launchctl", "rm -f \"$3\"");

    let err = uninstall_unit(&spec, &program_set(&vanishing), false, TIMEOUT_MS)
        .await
        .unwrap_err()
        .to_string();

    assert!(err.contains("cannot write"), "got: {err}");
}

#[tokio::test]
async fn an_uninstall_the_manager_refuses_still_removes_the_unit() {
    let dir = tempfile::tempdir().expect("temp dir");
    let spec = installed_spec(dir.path(), UnitKind::Launchd);
    uninstall_unit(&spec, &program_set(FALSE_PROGRAM), false, TIMEOUT_MS)
        .await
        .expect("a refusal to unload must not strand the unit file");
    assert!(!spec.unit_path().is_file());
}

#[tokio::test]
async fn an_uninstall_the_manager_refuses_says_what_it_skipped() {
    let dir = tempfile::tempdir().expect("temp dir");
    let spec = installed_spec(dir.path(), UnitKind::Launchd);
    let report = uninstall_unit(&spec, &program_set(FALSE_PROGRAM), false, TIMEOUT_MS)
        .await
        .expect("a refusal to unload must not strand the unit file");
    assert!(report.contains("skipped: "), "got: {report}");
}

#[tokio::test]
async fn a_status_report_that_cannot_reach_the_manager_is_reported() {
    let dir = tempfile::tempdir().expect("temp dir");
    let spec = installed_spec(dir.path(), UnitKind::Launchd);
    let err = status_report(
        &spec,
        &program_set(crate::unit_specs::MISSING_PROGRAM),
        TIMEOUT_MS,
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(err.starts_with("cannot run '/nonexistent/"), "got: {err}");
}

#[tokio::test]
async fn the_status_report_names_the_label_the_kind_and_the_state() {
    let dir = tempfile::tempdir().expect("temp dir");
    let spec = spec_for(UnitKind::Launchd, dir.path());
    let report = status_report(&spec, &program_set(FALSE_PROGRAM), TIMEOUT_MS)
        .await
        .expect("an absent unit needs no manager");
    assert!(
        report.starts_with("pm3-test (launchd service): not installed"),
        "got: {report}"
    );
}

#[tokio::test]
async fn the_status_report_sees_a_running_service() {
    let dir = tempfile::tempdir().expect("temp dir");
    let spec = installed_spec(dir.path(), UnitKind::Systemd);
    let program = fake_program(dir.path(), "systemctl", "echo active");
    let report = status_report(&spec, &program_set(&program), TIMEOUT_MS)
        .await
        .expect("the probe should be readable");
    assert!(
        report.contains("systemd service): running"),
        "got: {report}"
    );
}
