use super::*;
use crate::service_specs::spec_for;

#[test]
fn launchd_units_live_in_the_launch_agents_directory() {
    let dir = unit_dir_of(ServiceKind::Launchd, Path::new("/home/dev"));
    assert_eq!(dir, PathBuf::from("/home/dev/Library/LaunchAgents"));
}

#[test]
fn systemd_units_live_in_the_user_unit_directory() {
    let dir = unit_dir_of(ServiceKind::Systemd, Path::new("/home/dev"));
    assert_eq!(dir, PathBuf::from("/home/dev/.config/systemd/user"));
}

#[test]
fn a_launchd_unit_is_named_after_the_label_with_a_plist_suffix() {
    let spec = spec_for(ServiceKind::Launchd, Path::new("/home/dev"));
    assert_eq!(spec.unit_name(), "pm3-test.plist");
}

#[test]
fn a_systemd_unit_is_named_after_the_label_with_a_service_suffix() {
    let spec = spec_for(ServiceKind::Systemd, Path::new("/home/dev"));
    assert_eq!(spec.unit_name(), "pm3-test.service");
}

#[test]
fn the_unit_path_joins_the_directory_and_the_unit_name() {
    let spec = spec_for(ServiceKind::Launchd, Path::new("/home/dev"));
    assert_eq!(
        spec.unit_path(),
        PathBuf::from("/home/dev/Library/LaunchAgents/pm3-test.plist")
    );
}

#[test]
fn the_daemon_args_carry_the_absolute_config_path() {
    let spec = spec_for(ServiceKind::Systemd, Path::new("/home/dev"));
    assert_eq!(
        spec.daemon_args(),
        [
            "daemon".to_string(),
            "--config".to_string(),
            "/home/dev/.pm3/config.yaml".to_string()
        ]
    );
}

#[test]
fn each_service_kind_reports_its_own_name() {
    assert_eq!(ServiceKind::Launchd.as_str(), "launchd");
    assert_eq!(ServiceKind::Systemd.as_str(), "systemd");
}

#[test]
fn each_status_reports_its_own_name() {
    assert_eq!(ServiceStatus::NotInstalled.as_str(), "not installed");
    assert_eq!(
        ServiceStatus::InstalledNotRunning.as_str(),
        "installed, not running"
    );
    assert_eq!(ServiceStatus::Running.as_str(), "running");
}

#[test]
fn launchd_reports_running_when_the_listing_carries_a_pid() {
    assert!(parse_run_state(
        ServiceKind::Launchd,
        true,
        "{\n\t\"PID\" = 4242;\n}"
    ));
}

#[test]
fn launchd_reports_stopped_when_the_listing_carries_no_pid() {
    assert!(!parse_run_state(
        ServiceKind::Launchd,
        true,
        "{\n\t\"LastExitStatus\" = 0;\n}"
    ));
}

#[test]
fn launchd_reports_stopped_when_the_listing_command_failed() {
    assert!(!parse_run_state(
        ServiceKind::Launchd,
        false,
        "{\n\t\"PID\" = 4242;\n}"
    ));
}

#[test]
fn systemd_reports_running_for_an_active_unit() {
    assert!(parse_run_state(ServiceKind::Systemd, true, "active\n"));
}

#[test]
fn systemd_reports_stopped_for_an_inactive_unit() {
    assert!(!parse_run_state(ServiceKind::Systemd, true, "inactive\n"));
}

#[test]
fn systemd_reports_stopped_for_empty_output() {
    assert!(!parse_run_state(ServiceKind::Systemd, true, ""));
}
