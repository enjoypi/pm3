use super::*;
use crate::service_specs::program_set;

const FAKE: &str = "/tmp/pm3-fake-manager";
const UNIT_PATH: &str = "/home/dev/Library/LaunchAgents/pm3-test.plist";
const UNIT_NAME: &str = "pm3-test.service";

fn programs() -> ServiceProgramSet {
    program_set(FAKE)
}

#[test]
fn the_program_set_reads_every_manager_path_from_the_config() {
    let service = crate::config::ServiceConfig {
        label: "pm3-test".to_string(),
        restart_delay_secs: 2,
        restart_condition: "always".to_string(),
        launchctl_path: "/opt/launchctl".to_string(),
        systemctl_path: "/opt/systemctl".to_string(),
        loginctl_path: "/opt/loginctl".to_string(),
    };
    let programs = ServiceProgramSet::from_config(&service);
    assert_eq!(programs.launchctl, "/opt/launchctl");
    assert_eq!(programs.systemctl, "/opt/systemctl");
    assert_eq!(programs.loginctl, "/opt/loginctl");
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
