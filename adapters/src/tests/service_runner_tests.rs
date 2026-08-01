use std::path::{Path, PathBuf};

use super::*;
use crate::{
    ServiceKind,
    service_specs::{MISSING_PROGRAM, fake_program, program_set, spec_for},
};

const TRUE_PROGRAM: &str = "/usr/bin/true";
const FALSE_PROGRAM: &str = "/usr/bin/false";

fn run_step_of(program: &str) -> Vec<ServiceStep> {
    vec![ServiceStep::Run(ServiceCommand {
        program: program.to_string(),
        args: Vec::new(),
    })]
}

fn try_step_of(program: &str) -> Vec<ServiceStep> {
    vec![ServiceStep::TryRun(ServiceCommand {
        program: program.to_string(),
        args: Vec::new(),
    })]
}

fn write_step(dir: &Path, path: &Path) -> ServiceStep {
    ServiceStep::Write {
        dir: dir.to_path_buf(),
        path: path.to_path_buf(),
        contents: "unit body".to_string(),
    }
}

fn installed_spec(home: &Path, kind: ServiceKind) -> ServiceUnitSpec {
    let spec = spec_for(kind, home);
    std::fs::create_dir_all(&spec.unit_dir).expect("prepare the unit directory");
    std::fs::write(spec.unit_path(), "unit body").expect("install a unit file");
    spec
}

#[tokio::test]
async fn a_clean_exit_finishes_the_plan() {
    execute_plan(&run_step_of(TRUE_PROGRAM))
        .await
        .expect("a clean exit should finish the plan");
}

#[tokio::test]
async fn a_missing_program_stops_the_plan() {
    let err = execute_plan(&run_step_of(MISSING_PROGRAM))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.starts_with("cannot run '/nonexistent/"), "got: {err}");
}

#[tokio::test]
async fn a_silent_failure_reports_the_exit_status() {
    let err = execute_plan(&run_step_of(FALSE_PROGRAM))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("exited with status 1"), "got: {err}");
}

#[tokio::test]
async fn a_noisy_failure_reports_what_the_manager_said() {
    let dir = tempfile::tempdir().expect("temp dir");
    let program = fake_program(dir.path(), "noisy", "echo 'no such unit' >&2; exit 1");
    let err = execute_plan(&run_step_of(&program))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("no such unit"), "got: {err}");
}

#[tokio::test]
async fn an_optional_step_that_succeeds_skips_nothing() {
    let skipped = execute_plan(&try_step_of(TRUE_PROGRAM))
        .await
        .expect("a clean exit should finish the plan");
    assert!(skipped.is_empty(), "got: {skipped:?}");
}

#[tokio::test]
async fn an_optional_step_that_fails_is_reported_as_skipped() {
    let skipped = execute_plan(&try_step_of(FALSE_PROGRAM))
        .await
        .expect("an optional step must not fail the plan");
    let note = skipped.first().expect("the refusal should be noted");
    assert!(note.contains("exited with status 1"), "got: {note}");
}

#[tokio::test]
async fn an_optional_step_that_fails_lets_later_steps_run() {
    let dir = tempfile::tempdir().expect("temp dir");
    let unit_path = dir.path().join("late.plist");
    let mut steps = try_step_of(FALSE_PROGRAM);
    steps.push(write_step(dir.path(), &unit_path));
    execute_plan(&steps)
        .await
        .expect("an optional step must not fail the plan");
    assert!(unit_path.exists(), "later steps must still run");
}

#[tokio::test]
async fn writing_a_unit_creates_the_parent_directory() {
    let dir = tempfile::tempdir().expect("temp dir");
    let unit_dir = dir.path().join("Library/LaunchAgents");
    let unit_path = unit_dir.join("pm3-test.plist");
    execute_plan(&[write_step(&unit_dir, &unit_path)])
        .await
        .expect("the unit should be written");
    assert_eq!(
        std::fs::read_to_string(&unit_path).expect("read the unit"),
        "unit body"
    );
}

#[tokio::test]
async fn a_unit_directory_blocked_by_a_file_stops_the_plan() {
    let dir = tempfile::tempdir().expect("temp dir");
    let blocked = dir.path().join("blocked");
    std::fs::write(&blocked, "occupied").expect("occupy the unit directory");
    let err = execute_plan(&[write_step(&blocked, &blocked.join("pm3.plist"))])
        .await
        .unwrap_err()
        .to_string();
    assert!(err.starts_with("cannot write "), "got: {err}");
}

#[tokio::test]
async fn a_unit_path_blocked_by_a_directory_stops_the_plan() {
    let dir = tempfile::tempdir().expect("temp dir");
    let blocked = dir.path().join("pm3.plist");
    std::fs::create_dir(&blocked).expect("occupy the unit path");
    let err = execute_plan(&[write_step(dir.path(), &blocked)])
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("pm3.plist"), "got: {err}");
}

#[tokio::test]
async fn removing_a_unit_that_is_not_there_stops_the_plan() {
    let dir = tempfile::tempdir().expect("temp dir");
    let err = execute_plan(&[ServiceStep::Remove {
        path: dir.path().join("absent.plist"),
    }])
    .await
    .unwrap_err()
    .to_string();
    assert!(err.contains("absent.plist"), "got: {err}");
}

#[tokio::test]
async fn removing_an_installed_unit_finishes_the_plan() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("pm3-test.plist");
    std::fs::write(&path, "unit body").expect("install a unit file");
    execute_plan(&[ServiceStep::Remove { path: path.clone() }])
        .await
        .expect("the unit should be removed");
    assert!(!path.exists(), "the unit file should be gone");
}

#[tokio::test]
async fn the_plan_stops_at_the_first_failing_step() {
    let dir = tempfile::tempdir().expect("temp dir");
    let unit_path = dir.path().join("late.plist");
    let mut steps = run_step_of(FALSE_PROGRAM);
    steps.push(write_step(dir.path(), &unit_path));
    execute_plan(&steps)
        .await
        .expect_err("the failing step should stop the plan");
    assert!(!unit_path.exists(), "later steps must not run");
}

#[tokio::test]
async fn a_failing_plan_backs_out_the_files_it_created() {
    let dir = tempfile::tempdir().expect("temp dir");
    let unit = dir.path().join("units").join("pm3.service");
    let steps = vec![
        write_step(unit.parent().expect("a parent"), &unit),
        ServiceStep::Run(ServiceCommand {
            program: FALSE_PROGRAM.to_string(),
            args: Vec::new(),
        }),
    ];

    execute_plan(&steps)
        .await
        .expect_err("the plan should fail");

    assert!(
        !unit.exists(),
        "a half-installed unit would make the status query lie"
    );
}

#[tokio::test]
async fn a_failing_plan_leaves_a_file_it_only_overwrote() {
    let dir = tempfile::tempdir().expect("temp dir");
    let unit = dir.path().join("pm3.service");
    std::fs::write(&unit, "the operator's own unit").expect("seed the unit");
    let steps = vec![
        write_step(dir.path(), &unit),
        ServiceStep::Run(ServiceCommand {
            program: FALSE_PROGRAM.to_string(),
            args: Vec::new(),
        }),
    ];

    execute_plan(&steps)
        .await
        .expect_err("the plan should fail");

    assert!(
        unit.exists(),
        "pm3 must not delete a file it did not create"
    );
}

#[tokio::test]
async fn an_absent_unit_file_means_the_service_is_not_installed() {
    let dir = tempfile::tempdir().expect("temp dir");
    let spec = spec_for(ServiceKind::Launchd, dir.path());
    let status = query_status(&spec, &program_set(MISSING_PROGRAM))
        .await
        .expect("an absent unit needs no manager");
    assert_eq!(status, ServiceStatus::NotInstalled);
}

#[tokio::test]
async fn a_missing_manager_stops_the_status_query() {
    let dir = tempfile::tempdir().expect("temp dir");
    let spec = installed_spec(dir.path(), ServiceKind::Launchd);
    let err = query_status(&spec, &program_set(MISSING_PROGRAM))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.starts_with("cannot run '/nonexistent/"), "got: {err}");
}

#[tokio::test]
async fn a_launch_agent_with_a_pid_is_running() {
    let dir = tempfile::tempdir().expect("temp dir");
    let spec = installed_spec(dir.path(), ServiceKind::Launchd);
    let program = fake_program(dir.path(), "launchctl", "echo '\"PID\" = 4242;'");
    let status = query_status(&spec, &program_set(&program))
        .await
        .expect("the listing should be readable");
    assert_eq!(status, ServiceStatus::Running);
}

#[tokio::test]
async fn a_launch_agent_without_a_pid_is_installed_but_stopped() {
    let dir = tempfile::tempdir().expect("temp dir");
    let spec = installed_spec(dir.path(), ServiceKind::Launchd);
    let program = fake_program(dir.path(), "launchctl", "echo 'LastExitStatus = 0;'");
    let status = query_status(&spec, &program_set(&program))
        .await
        .expect("the listing should be readable");
    assert_eq!(status, ServiceStatus::InstalledNotRunning);
}

#[tokio::test]
async fn an_active_systemd_unit_is_running() {
    let dir = tempfile::tempdir().expect("temp dir");
    let spec = installed_spec(dir.path(), ServiceKind::Systemd);
    let program = fake_program(dir.path(), "systemctl", "echo active");
    let status = query_status(&spec, &program_set(&program))
        .await
        .expect("the probe should be readable");
    assert_eq!(status, ServiceStatus::Running);
}

#[tokio::test]
async fn an_inactive_systemd_unit_is_installed_but_stopped() {
    let dir = tempfile::tempdir().expect("temp dir");
    let spec = installed_spec(dir.path(), ServiceKind::Systemd);
    let program = fake_program(dir.path(), "systemctl", "echo inactive; exit 3");
    let status = query_status(&spec, &program_set(&program))
        .await
        .expect("a stopped unit is not an error");
    assert_eq!(status, ServiceStatus::InstalledNotRunning);
}

#[test]
fn every_error_variant_renders_a_message() {
    let errors = [
        ServiceCommandError::Spawn {
            program: "launchctl".to_string(),
            reason: "not found".to_string(),
        },
        ServiceCommandError::Failed {
            program: "launchctl".to_string(),
            reason: "exited with status 1".to_string(),
        },
        ServiceCommandError::Io {
            path: PathBuf::from("/tmp/pm3.plist").display().to_string(),
            reason: "permission denied".to_string(),
        },
    ];
    for err in errors {
        assert!(
            err.to_string().starts_with("cannot "),
            "error message must start with a verb: {err}"
        );
    }
}
