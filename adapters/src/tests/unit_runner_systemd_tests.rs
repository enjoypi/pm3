#![cfg(unix)]
use super::*;

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
async fn schtasks_never_asks_about_linger() {
    let state = linger_state(
        UnitKind::WinSchtasks,
        &program_set(MISSING_PROGRAM),
        TIMEOUT_MS,
    )
    .await;
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

#[tokio::test]
async fn a_launchd_supervised_pid_comes_from_the_listing() {
    let dir = tempfile::tempdir().expect("temp dir");
    let spec = spec_for(UnitKind::Launchd, dir.path());
    let program = fake_program(dir.path(), "launchctl", "echo '\"PID\" = 4242;'");
    let pid = query_supervised_pid(&spec, &program_set(&program), TIMEOUT_MS)
        .await
        .expect("the listing should be readable");
    assert_eq!(pid, Some(4242));
}

#[tokio::test]
async fn a_systemd_supervised_pid_comes_from_the_main_pid_property() {
    let dir = tempfile::tempdir().expect("temp dir");
    let spec = spec_for(UnitKind::Systemd, dir.path());
    let program = fake_program(dir.path(), "systemctl", "echo 4242");
    let pid = query_supervised_pid(&spec, &program_set(&program), TIMEOUT_MS)
        .await
        .expect("the property should be readable");
    assert_eq!(pid, Some(4242));
}

#[tokio::test]
async fn a_zero_main_pid_means_nothing_is_supervised() {
    let dir = tempfile::tempdir().expect("temp dir");
    let spec = spec_for(UnitKind::Systemd, dir.path());
    let program = fake_program(dir.path(), "systemctl", "echo 0");
    let pid = query_supervised_pid(&spec, &program_set(&program), TIMEOUT_MS)
        .await
        .expect("a zero answer is not an error");
    assert_eq!(pid, None);
}

#[tokio::test]
async fn a_refused_pid_query_means_nothing_is_supervised() {
    let dir = tempfile::tempdir().expect("temp dir");
    let spec = spec_for(UnitKind::Systemd, dir.path());
    let pid = query_supervised_pid(&spec, &program_set(FALSE_PROGRAM), TIMEOUT_MS)
        .await
        .expect("a refused query is not an error");
    assert_eq!(pid, None);
}

#[tokio::test]
async fn a_schtasks_task_has_no_manager_pid_to_read() {
    let dir = tempfile::tempdir().expect("temp dir");
    let spec = spec_for(UnitKind::WinSchtasks, dir.path());
    let pid = query_supervised_pid(&spec, &program_set(TRUE_PROGRAM), TIMEOUT_MS)
        .await
        .expect("a successful query is not an error");
    assert_eq!(pid, None);
}

#[tokio::test]
async fn a_missing_pid_query_program_is_an_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let spec = spec_for(UnitKind::Systemd, dir.path());
    let err = query_supervised_pid(&spec, &program_set(MISSING_PROGRAM), TIMEOUT_MS)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.starts_with("cannot run '/nonexistent/"), "got: {err}");
}

#[tokio::test]
async fn a_hand_back_kickstarts_the_agent() {
    let dir = tempfile::tempdir().expect("temp dir");
    let spec = spec_for(UnitKind::Launchd, dir.path());
    let program = fake_program(dir.path(), "launchctl", "printf '%s' \"$*\" > \"$0.args\"");
    let handed = hand_back_to_manager(
        &spec,
        &program_set_for_user(&program, OWNER_UID, OWNER_RUNTIME_DIR),
        TIMEOUT_MS,
    )
    .await
    .expect("a kickstart should succeed");
    assert!(handed);
    let recorded =
        std::fs::read_to_string(format!("{program}.args")).expect("the args were logged");
    assert_eq!(recorded, "kickstart gui/4242/pm3-test");
}

#[tokio::test]
async fn a_hand_back_without_a_known_uid_is_impossible() {
    let dir = tempfile::tempdir().expect("temp dir");
    let spec = spec_for(UnitKind::Launchd, dir.path());
    let handed = hand_back_to_manager(&spec, &program_set(TRUE_PROGRAM), TIMEOUT_MS)
        .await
        .expect("an unknown uid is not an error");
    assert!(!handed);
}

#[tokio::test]
async fn a_failed_hand_back_is_an_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let spec = spec_for(UnitKind::Launchd, dir.path());
    let err = hand_back_to_manager(
        &spec,
        &program_set_for_user(FALSE_PROGRAM, OWNER_UID, OWNER_RUNTIME_DIR),
        TIMEOUT_MS,
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(err.contains("cannot complete"), "got: {err}");
}
