use std::path::{Path, PathBuf};

use super::*;

const TIMEOUT_MS: u64 = 5000;
use crate::{
    UnitKind,
    unit_specs::{MISSING_PROGRAM, fake_program, program_set, program_set_for_user, spec_for},
};

const TRUE_PROGRAM: &str = "/usr/bin/true";
const FALSE_PROGRAM: &str = "/usr/bin/false";
const OWNER_UID: u32 = 4242;
const OWNER_RUNTIME_DIR: &str = "/run/user/4242";

fn bare_command(program: &str) -> UnitCommand {
    UnitCommand {
        program: program.to_string(),
        args: Vec::new(),
        env: Vec::new(),
    }
}

fn run_step_of(program: &str) -> Vec<UnitStep> {
    vec![UnitStep::Run(bare_command(program))]
}

fn try_step_of(program: &str) -> Vec<UnitStep> {
    vec![UnitStep::TryRun(bare_command(program))]
}

fn write_step(dir: &Path, path: &Path) -> UnitStep {
    UnitStep::Write {
        dir: dir.to_path_buf(),
        path: path.to_path_buf(),
        contents: "unit body".to_string(),
    }
}

fn installed_spec(home: &Path, kind: UnitKind) -> UnitSpec {
    let spec = spec_for(kind, home);
    std::fs::create_dir_all(&spec.unit_dir).expect("prepare the unit directory");
    std::fs::write(spec.unit_path(), "unit body").expect("install a unit file");
    spec
}

#[tokio::test]
async fn a_clean_exit_finishes_the_plan() {
    execute_plan(&run_step_of(TRUE_PROGRAM), TIMEOUT_MS)
        .await
        .expect("a clean exit should finish the plan");
}

#[tokio::test]
async fn a_missing_program_stops_the_plan() {
    let err = execute_plan(&run_step_of(MISSING_PROGRAM), TIMEOUT_MS)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.starts_with("cannot run '/nonexistent/"), "got: {err}");
}

#[tokio::test]
async fn a_silent_failure_reports_the_exit_status() {
    let err = execute_plan(&run_step_of(FALSE_PROGRAM), TIMEOUT_MS)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("exited with status 1"), "got: {err}");
}

#[tokio::test]
async fn a_noisy_failure_reports_what_the_manager_said() {
    let dir = tempfile::tempdir().expect("temp dir");
    let program = fake_program(dir.path(), "noisy", "echo 'no such unit' >&2; exit 1");
    let err = execute_plan(&run_step_of(&program), TIMEOUT_MS)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("no such unit"), "got: {err}");
}

#[tokio::test]
async fn an_optional_step_that_succeeds_skips_nothing() {
    let skipped = execute_plan(&try_step_of(TRUE_PROGRAM), TIMEOUT_MS)
        .await
        .expect("a clean exit should finish the plan");
    assert!(skipped.is_empty(), "got: {skipped:?}");
}

#[tokio::test]
async fn an_optional_step_that_fails_is_reported_as_skipped() {
    let skipped = execute_plan(&try_step_of(FALSE_PROGRAM), TIMEOUT_MS)
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
    execute_plan(&steps, TIMEOUT_MS)
        .await
        .expect("an optional step must not fail the plan");
    assert!(unit_path.exists(), "later steps must still run");
}

#[tokio::test]
async fn writing_a_unit_creates_the_parent_directory() {
    let dir = tempfile::tempdir().expect("temp dir");
    let unit_dir = dir.path().join("Library/LaunchAgents");
    let unit_path = unit_dir.join("pm3-test.plist");
    execute_plan(&[write_step(&unit_dir, &unit_path)], TIMEOUT_MS)
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
    let err = execute_plan(
        &[write_step(&blocked, &blocked.join("pm3.plist"))],
        TIMEOUT_MS,
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(err.starts_with("cannot write "), "got: {err}");
}

#[cfg(unix)]
#[tokio::test]
async fn a_unit_directory_that_cannot_be_created_stops_the_plan() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().expect("temp dir");
    let readonly = dir.path().join("readonly");
    std::fs::create_dir(&readonly).expect("create the readonly parent");
    std::fs::set_permissions(&readonly, std::fs::Permissions::from_mode(0o555))
        .expect("make the parent readonly");
    let unit_dir = readonly.join("units");
    let err = execute_plan(
        &[write_step(&unit_dir, &unit_dir.join("pm3.plist"))],
        TIMEOUT_MS,
    )
    .await
    .unwrap_err()
    .to_string();
    std::fs::set_permissions(&readonly, std::fs::Permissions::from_mode(0o755))
        .expect("restore the parent permissions");
    assert!(err.starts_with("cannot write "), "got: {err}");
}

#[tokio::test]
async fn a_unit_path_blocked_by_a_directory_stops_the_plan() {
    let dir = tempfile::tempdir().expect("temp dir");
    let blocked = dir.path().join("pm3.plist");
    std::fs::create_dir(&blocked).expect("occupy the unit path");
    let err = execute_plan(&[write_step(dir.path(), &blocked)], TIMEOUT_MS)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("pm3.plist"), "got: {err}");
}

#[tokio::test]
async fn removing_a_unit_that_is_not_there_stops_the_plan() {
    let dir = tempfile::tempdir().expect("temp dir");
    let err = execute_plan(
        &[UnitStep::Remove {
            path: dir.path().join("absent.plist"),
        }],
        TIMEOUT_MS,
    )
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
    execute_plan(&[UnitStep::Remove { path: path.clone() }], TIMEOUT_MS)
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
    execute_plan(&steps, TIMEOUT_MS)
        .await
        .expect_err("the failing step should stop the plan");
    assert!(!unit_path.exists(), "later steps must not run");
}

#[tokio::test]
async fn an_unreadable_unit_path_stops_the_plan_and_rolls_back_earlier_writes() {
    let dir = tempfile::tempdir().expect("temp dir");
    let blocker = dir.path().join("blocked");
    std::fs::write(&blocker, "occupied").expect("occupy the unit directory");
    let created = dir.path().join("created.plist");
    let steps = vec![
        write_step(dir.path(), &created),
        write_step(&blocker, &blocker.join("pm3.plist")),
    ];

    let err = execute_plan(&steps, TIMEOUT_MS)
        .await
        .unwrap_err()
        .to_string();

    assert!(err.contains("pm3.plist"), "got: {err}");
    assert!(!created.exists(), "the earlier write must be rolled back");
}

#[tokio::test]
async fn a_failing_plan_backs_out_the_files_it_created() {
    let dir = tempfile::tempdir().expect("temp dir");
    let unit = dir.path().join("units").join("pm3.service");
    let steps = vec![
        write_step(unit.parent().expect("a parent"), &unit),
        UnitStep::Run(bare_command(FALSE_PROGRAM)),
    ];

    execute_plan(&steps, TIMEOUT_MS)
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
        UnitStep::Run(bare_command(FALSE_PROGRAM)),
    ];

    execute_plan(&steps, TIMEOUT_MS)
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
    let spec = spec_for(UnitKind::Launchd, dir.path());
    let status = query_status(&spec, &program_set(MISSING_PROGRAM), TIMEOUT_MS)
        .await
        .expect("an absent unit needs no manager");
    assert_eq!(status, UnitStatus::NotInstalled);
}

#[tokio::test]
async fn a_missing_manager_stops_the_status_query() {
    let dir = tempfile::tempdir().expect("temp dir");
    let spec = installed_spec(dir.path(), UnitKind::Launchd);
    let err = query_status(&spec, &program_set(MISSING_PROGRAM), TIMEOUT_MS)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.starts_with("cannot run '/nonexistent/"), "got: {err}");
}

#[tokio::test]
async fn a_launch_agent_with_a_pid_is_running() {
    let dir = tempfile::tempdir().expect("temp dir");
    let spec = installed_spec(dir.path(), UnitKind::Launchd);
    let program = fake_program(dir.path(), "launchctl", "echo '\"PID\" = 4242;'");
    let status = query_status(&spec, &program_set(&program), TIMEOUT_MS)
        .await
        .expect("the listing should be readable");
    assert_eq!(status, UnitStatus::Running);
}

#[tokio::test]
async fn a_launch_agent_without_a_pid_is_installed_but_stopped() {
    let dir = tempfile::tempdir().expect("temp dir");
    let spec = installed_spec(dir.path(), UnitKind::Launchd);
    let program = fake_program(dir.path(), "launchctl", "echo 'LastExitStatus = 0;'");
    let status = query_status(&spec, &program_set(&program), TIMEOUT_MS)
        .await
        .expect("the listing should be readable");
    assert_eq!(status, UnitStatus::InstalledNotRunning);
}

#[tokio::test]
async fn an_active_systemd_unit_is_running() {
    let dir = tempfile::tempdir().expect("temp dir");
    let spec = installed_spec(dir.path(), UnitKind::Systemd);
    let program = fake_program(dir.path(), "systemctl", "echo active");
    let status = query_status(&spec, &program_set(&program), TIMEOUT_MS)
        .await
        .expect("the probe should be readable");
    assert_eq!(status, UnitStatus::Running);
}

#[tokio::test]
async fn an_inactive_systemd_unit_is_installed_but_stopped() {
    let dir = tempfile::tempdir().expect("temp dir");
    let spec = installed_spec(dir.path(), UnitKind::Systemd);
    let program = fake_program(dir.path(), "systemctl", "echo inactive; exit 3");
    let status = query_status(&spec, &program_set(&program), TIMEOUT_MS)
        .await
        .expect("a stopped unit is not an error");
    assert_eq!(status, UnitStatus::InstalledNotRunning);
}

#[tokio::test]
async fn a_user_scoped_call_carries_the_runtime_directory() {
    let dir = tempfile::tempdir().expect("temp dir");
    let spec = installed_spec(dir.path(), UnitKind::Systemd);
    let program = fake_program(
        dir.path(),
        "systemctl",
        &format!("test \"$XDG_RUNTIME_DIR\" = \"{OWNER_RUNTIME_DIR}\" && echo active"),
    );
    let programs = program_set_for_user(&program, OWNER_UID, OWNER_RUNTIME_DIR);
    let status = query_status(&spec, &programs, TIMEOUT_MS)
        .await
        .expect("the probe should be readable");
    assert_eq!(
        status,
        UnitStatus::Running,
        "a session without a bus cannot reach the user manager"
    );
}

#[tokio::test]
async fn a_lingering_user_needs_no_further_permission() {
    let dir = tempfile::tempdir().expect("temp dir");
    let program = fake_program(dir.path(), "loginctl", "echo yes");
    let programs = program_set_for_user(&program, OWNER_UID, OWNER_RUNTIME_DIR);
    let state = linger_state(UnitKind::Systemd, &programs, TIMEOUT_MS).await;
    assert_eq!(state, LingerState::Enabled);
}

#[tokio::test]
async fn a_user_without_linger_leaves_the_state_unknown() {
    let dir = tempfile::tempdir().expect("temp dir");
    let program = fake_program(dir.path(), "loginctl", "echo no");
    let programs = program_set_for_user(&program, OWNER_UID, OWNER_RUNTIME_DIR);
    let state = linger_state(UnitKind::Systemd, &programs, TIMEOUT_MS).await;
    assert_eq!(state, LingerState::Unknown);
}

#[tokio::test]
async fn a_manager_that_cannot_be_asked_leaves_the_state_unknown() {
    let programs = program_set_for_user(MISSING_PROGRAM, OWNER_UID, OWNER_RUNTIME_DIR);
    let state = linger_state(UnitKind::Systemd, &programs, TIMEOUT_MS).await;
    assert_eq!(state, LingerState::Unknown);
}

#[tokio::test]
async fn an_unknown_owner_leaves_the_state_unknown() {
    let state = linger_state(UnitKind::Systemd, &program_set(TRUE_PROGRAM), TIMEOUT_MS).await;
    assert_eq!(state, LingerState::Unknown);
}

#[tokio::test]
async fn launchd_never_asks_about_linger() {
    let state = linger_state(UnitKind::Launchd, &program_set(MISSING_PROGRAM), TIMEOUT_MS).await;
    assert_eq!(state, LingerState::Unknown);
}

#[tokio::test]
async fn a_manager_that_never_answers_is_given_up_on() {
    let dir = tempfile::tempdir().expect("temp dir");
    let program = fake_program(dir.path(), "slow-systemctl", "sleep 30");
    let err = execute_plan(&run_step_of(&program), 30)
        .await
        .expect_err("a stalled manager fails the plan");
    assert!(err.to_string().contains("within 30 ms"), "got: {err}");
}

#[test]
fn every_error_variant_renders_a_message() {
    let errors = [
        UnitCommandError::Stalled {
            program: "systemctl".to_string(),
            timeout_ms: 30,
        },
        UnitCommandError::Spawn {
            program: "launchctl".to_string(),
            reason: "not found".to_string(),
        },
        UnitCommandError::Failed {
            program: "launchctl".to_string(),
            reason: "exited with status 1".to_string(),
        },
        UnitCommandError::Io {
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
