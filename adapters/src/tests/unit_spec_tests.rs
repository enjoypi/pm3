use super::*;
use crate::unit_specs::spec_for;

#[test]
fn launchd_units_live_in_the_launch_agents_directory() {
    let dir = unit_dir_of(UnitKind::Launchd, Path::new("/home/dev"));
    assert_eq!(dir, PathBuf::from("/home/dev/Library/LaunchAgents"));
}

#[test]
fn systemd_units_live_in_the_user_unit_directory() {
    let dir = unit_dir_of(UnitKind::Systemd, Path::new("/home/dev"));
    assert_eq!(dir, PathBuf::from("/home/dev/.config/systemd/user"));
}

#[test]
fn a_launchd_unit_is_named_after_the_label_with_a_plist_suffix() {
    let spec = spec_for(UnitKind::Launchd, Path::new("/home/dev"));
    assert_eq!(spec.unit_name(), "pm3-test.plist");
}

#[test]
fn a_systemd_unit_is_named_after_the_label_with_a_service_suffix() {
    let spec = spec_for(UnitKind::Systemd, Path::new("/home/dev"));
    assert_eq!(spec.unit_name(), "pm3-test.service");
}

#[test]
fn the_unit_path_joins_the_directory_and_the_unit_name() {
    let spec = spec_for(UnitKind::Launchd, Path::new("/home/dev"));
    assert_eq!(
        spec.unit_path(),
        PathBuf::from("/home/dev/Library/LaunchAgents/pm3-test.plist")
    );
}

#[test]
fn the_daemon_args_carry_the_absolute_config_path() {
    let spec = spec_for(UnitKind::Systemd, Path::new("/home/dev"));
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
    assert_eq!(UnitKind::Launchd.as_str(), "launchd");
    assert_eq!(UnitKind::Systemd.as_str(), "systemd");
    assert_eq!(UnitKind::WinSchtasks.as_str(), "schtasks");
}

#[test]
fn each_status_reports_its_own_name() {
    assert_eq!(UnitStatus::NotInstalled.as_str(), "not installed");
    assert_eq!(
        UnitStatus::InstalledNotRunning.as_str(),
        "installed, not running"
    );
    assert_eq!(UnitStatus::Running.as_str(), "running");
}

#[test]
fn launchd_reports_running_when_the_listing_carries_a_pid() {
    assert!(parse_run_state(
        UnitKind::Launchd,
        true,
        "{\n\t\"PID\" = 4242;\n}"
    ));
}

#[test]
fn launchd_reports_stopped_when_the_listing_carries_no_pid() {
    assert!(!parse_run_state(
        UnitKind::Launchd,
        true,
        "{\n\t\"LastExitStatus\" = 0;\n}"
    ));
}

#[test]
fn launchd_reports_stopped_when_the_listing_command_failed() {
    assert!(!parse_run_state(
        UnitKind::Launchd,
        false,
        "{\n\t\"PID\" = 4242;\n}"
    ));
}

#[test]
fn systemd_reports_running_for_an_active_unit() {
    assert!(parse_run_state(UnitKind::Systemd, true, "active\n"));
}

#[test]
fn systemd_reports_stopped_for_an_inactive_unit() {
    assert!(!parse_run_state(UnitKind::Systemd, true, "inactive\n"));
}

#[test]
fn systemd_reports_stopped_for_empty_output() {
    assert!(!parse_run_state(UnitKind::Systemd, true, ""));
}

#[test]
fn schtasks_units_live_next_to_the_runtime_state() {
    let dir = unit_dir_of(UnitKind::WinSchtasks, Path::new("/home/dev"));
    assert_eq!(dir, PathBuf::from("/home/dev/.pm3/service"));
}

#[test]
fn a_schtasks_unit_is_an_xml_file_with_a_cmd_wrapper_beside_it() {
    let spec = spec_for(UnitKind::WinSchtasks, Path::new("/home/dev"));
    assert_eq!(spec.unit_name(), "pm3-test.xml");
    assert_eq!(spec.wrapper_name(), "pm3-test-daemon.cmd");
    assert_eq!(
        spec.wrapper_path(),
        PathBuf::from("/home/dev/.pm3/service/pm3-test-daemon.cmd")
    );
}

#[test]
fn schtasks_reports_running_when_the_listing_says_so() {
    assert!(parse_run_state(
        UnitKind::WinSchtasks,
        true,
        "Status: Running\r\n"
    ));
}

#[test]
fn schtasks_reports_stopped_for_a_ready_task() {
    assert!(!parse_run_state(
        UnitKind::WinSchtasks,
        true,
        "Status: Ready\r\n"
    ));
}

#[test]
fn schtasks_reports_stopped_when_the_query_failed() {
    assert!(!parse_run_state(
        UnitKind::WinSchtasks,
        false,
        "Status: Running\r\n"
    ));
}

#[test]
fn a_lingering_user_is_read_from_the_property_value() {
    assert_eq!(parse_linger_state(true, "yes\n"), LingerState::Enabled);
}

#[test]
fn a_user_that_does_not_linger_is_read_as_unknown() {
    assert_eq!(parse_linger_state(true, "no\n"), LingerState::Unknown);
}

#[test]
fn a_refused_linger_query_is_read_as_unknown() {
    assert_eq!(parse_linger_state(false, "yes\n"), LingerState::Unknown);
}

#[test]
fn a_declared_runtime_directory_wins() {
    assert_eq!(
        runtime_dir_of(Some("/run/user/1000"), Some(4242)),
        Some("/run/user/1000".to_string())
    );
}

#[test]
fn an_undeclared_runtime_directory_follows_the_uid() {
    assert_eq!(
        runtime_dir_of(None, Some(4242)),
        Some("/run/user/4242".to_string()),
        "a non-login shell has no XDG_RUNTIME_DIR, yet systemctl --user needs one"
    );
}

#[test]
fn an_empty_runtime_directory_follows_the_uid() {
    assert_eq!(
        runtime_dir_of(Some(""), Some(4242)),
        Some("/run/user/4242".to_string())
    );
}

#[test]
fn an_unknown_owner_has_no_runtime_directory() {
    assert_eq!(runtime_dir_of(None, None), None);
}

#[test]
fn a_launchd_listing_yields_its_pid() {
    let listing = "{\n\t\"PID\" = 4242;\n\t\"Label\" = \"pm3-test\";\n};\n";
    assert_eq!(parse_launchd_pid(listing), Some(4242));
}

#[test]
fn a_launchd_listing_without_a_pid_yields_nothing() {
    assert_eq!(
        parse_launchd_pid("{\n\t\"Label\" = \"pm3-test\";\n};\n"),
        None
    );
}

#[test]
fn a_launchd_listing_with_an_unreadable_pid_yields_nothing() {
    assert_eq!(parse_launchd_pid("\"PID\" = soon;"), None);
}

#[test]
fn a_main_pid_is_parsed_from_the_value_line() {
    assert_eq!(parse_main_pid("4242\n"), Some(4242));
}

#[test]
fn a_zero_main_pid_means_unsupervised() {
    assert_eq!(parse_main_pid("0\n"), None);
}

#[test]
fn an_unreadable_main_pid_means_unsupervised() {
    assert_eq!(parse_main_pid("n/a"), None);
}
