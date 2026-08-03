use super::*;
use crate::unit_specs::{program_set, program_set_for_user};

const FAKE: &str = "/tmp/pm3-fake-manager";
const UNIT_PATH: &str = "/home/dev/Library/LaunchAgents/pm3-test.plist";
const UNIT_NAME: &str = "pm3-test.service";
const OWNER_UID: u32 = 4242;
const OWNER_RUNTIME_DIR: &str = "/run/user/4242";

fn programs() -> UnitProgramSet {
    program_set(FAKE)
}

fn owned_programs() -> UnitProgramSet {
    program_set_for_user(FAKE, OWNER_UID, OWNER_RUNTIME_DIR)
}

fn service_config() -> crate::config::ServiceConfig {
    crate::config::ServiceConfig {
        label: "pm3-test".to_string(),
        restart_delay_secs: 2,
        restart_condition: "always".to_string(),
        launchctl_path: "/opt/launchctl".to_string(),
        systemctl_path: "/opt/systemctl".to_string(),
        loginctl_path: "/opt/loginctl".to_string(),
    }
}

#[test]
fn the_program_set_reads_every_manager_path_from_the_config() {
    let programs = UnitProgramSet::from_config(&service_config(), None, None);
    assert_eq!(programs.launchctl, "/opt/launchctl");
    assert_eq!(programs.systemctl, "/opt/systemctl");
    assert_eq!(programs.loginctl, "/opt/loginctl");
}

#[test]
fn the_program_set_remembers_the_session_the_host_gave_it() {
    let programs =
        UnitProgramSet::from_config(&service_config(), Some(OWNER_RUNTIME_DIR), Some(OWNER_UID));
    assert_eq!(programs.runtime_dir.as_deref(), Some(OWNER_RUNTIME_DIR));
    assert_eq!(programs.uid, Some(OWNER_UID));
}

#[test]
fn loading_a_launch_agent_overrides_the_disabled_flag() {
    let command = launchctl_load(&programs(), Path::new(UNIT_PATH));
    assert_eq!(command.program, FAKE);
    assert_eq!(command.args, ["load", "-w", UNIT_PATH]);
}

#[test]
fn unloading_a_launch_agent_overrides_the_disabled_flag() {
    let command = launchctl_unload(&programs(), Path::new(UNIT_PATH));
    assert_eq!(command.args, ["unload", "-w", UNIT_PATH]);
}

#[test]
fn listing_a_launch_agent_targets_the_label() {
    let command = launchctl_list(&programs(), "pm3-test");
    assert_eq!(command.args, ["list", "pm3-test"]);
}

#[test]
fn reloading_systemd_takes_no_unit_argument() {
    let command = systemctl_daemon_reload(&programs());
    assert_eq!(command.args, ["--user", "daemon-reload"]);
}

#[test]
fn enabling_a_systemd_unit_starts_it_immediately() {
    let command = systemctl_enable_now(&programs(), UNIT_NAME);
    assert_eq!(command.args, ["--user", "enable", "--now", UNIT_NAME]);
}

#[test]
fn disabling_a_systemd_unit_stops_it_immediately() {
    let command = systemctl_disable_now(&programs(), UNIT_NAME);
    assert_eq!(command.args, ["--user", "disable", "--now", UNIT_NAME]);
}

#[test]
fn asking_systemd_for_liveness_targets_the_unit_name() {
    let command = systemctl_is_active(&programs(), UNIT_NAME);
    assert_eq!(command.args, ["--user", "is-active", UNIT_NAME]);
}

#[test]
fn enabling_linger_defaults_to_the_current_user() {
    let command = loginctl_enable_linger(&programs());
    assert_eq!(command.program, FAKE);
    assert_eq!(command.args, ["enable-linger"]);
}

#[test]
fn a_user_scoped_call_exports_the_runtime_directory() {
    let command = systemctl_daemon_reload(&owned_programs());
    assert_eq!(
        command.env,
        [("XDG_RUNTIME_DIR".to_string(), OWNER_RUNTIME_DIR.to_string())]
    );
}

#[test]
fn a_user_scoped_call_without_a_known_session_exports_nothing() {
    let command = systemctl_daemon_reload(&programs());
    assert!(command.env.is_empty(), "got: {:?}", command.env);
}

#[test]
fn a_launch_agent_call_needs_no_runtime_directory() {
    let command = launchctl_list(&owned_programs(), "pm3-test");
    assert!(command.env.is_empty(), "got: {:?}", command.env);
}

#[test]
fn reading_linger_targets_the_owning_uid() {
    let command = loginctl_show_linger(&owned_programs()).expect("a known uid can be asked about");
    assert_eq!(command.program, FAKE);
    assert_eq!(
        command.args,
        ["show-user", "4242", "-p", "Linger", "--value"]
    );
}

#[test]
fn linger_cannot_be_read_for_an_unknown_uid() {
    assert!(
        loginctl_show_linger(&programs()).is_none(),
        "loginctl show-user without a user reports nothing at all"
    );
}
