use std::path::Path;

use super::*;
use crate::{
    UnitKind,
    unit_specs::{MAX_TASKS, PM3_HOME_VALUE, PM3_HOME_VARIABLE, spec_for},
};

fn rendered() -> String {
    render_unit(&spec_for(UnitKind::Systemd, Path::new("/home/dev")))
}

#[test]
fn the_unit_declares_all_three_sections() {
    let unit = rendered();
    assert!(unit.contains("[Unit]"), "got: {unit}");
    assert!(unit.contains("[Service]"), "got: {unit}");
    assert!(unit.contains("[Install]"), "got: {unit}");
}

#[test]
fn the_unit_describes_itself_with_the_label() {
    assert!(
        rendered().contains("Description=pm3-test"),
        "got: {}",
        rendered()
    );
}

#[test]
fn the_unit_quotes_every_exec_start_token() {
    let expected =
        "ExecStart=\"/usr/local/bin/pm3\" \"daemon\" \"--config\" \"/home/dev/.pm3/config.yaml\"";
    assert!(rendered().contains(expected), "got: {}", rendered());
}

#[test]
fn the_unit_takes_the_restart_condition_from_the_spec() {
    let unit = rendered();
    assert!(unit.contains("Restart=always"), "got: {unit}");
    assert!(unit.contains("RestartSec=2"), "got: {unit}");
}

#[test]
fn the_unit_restarts_only_on_failure_when_the_config_says_so() {
    let mut spec = spec_for(UnitKind::Systemd, Path::new("/home/dev"));
    spec.restart_condition = "on-failure".to_string();
    let unit = render_unit(&spec);
    assert!(unit.contains("Restart=on-failure"), "got: {unit}");
}

#[test]
fn the_unit_takes_the_restart_delay_from_the_spec() {
    let mut spec = spec_for(UnitKind::Systemd, Path::new("/home/dev"));
    spec.restart_delay_secs = 9;
    assert!(
        render_unit(&spec).contains("RestartSec=9"),
        "got: {}",
        render_unit(&spec)
    );
}

#[test]
fn the_unit_kills_only_the_daemon_and_not_its_whole_control_group() {
    assert!(
        rendered().contains("KillMode=process"),
        "got: {}",
        rendered()
    );
}

#[test]
fn the_unit_keeps_every_file_the_daemon_writes_to_its_owner() {
    assert!(rendered().contains("UMask=0077"), "got: {}", rendered());
}

#[test]
fn the_unit_refuses_to_leave_a_core_dump_of_the_daemon() {
    assert!(rendered().contains("LimitCORE=0"), "got: {}", rendered());
}

#[test]
fn the_unit_appends_both_streams_to_the_daemon_log() {
    let unit = rendered();
    assert!(
        unit.contains("StandardOutput=append:/home/dev/.pm3/pm3.log"),
        "got: {unit}"
    );
    assert!(
        unit.contains("StandardError=append:/home/dev/.pm3/pm3.log"),
        "got: {unit}"
    );
}

#[test]
fn the_unit_forwards_the_search_path() {
    assert!(
        rendered().contains("Environment=\"PATH=/usr/bin:/bin\""),
        "got: {}",
        rendered()
    );
}

#[test]
fn the_unit_wants_to_start_with_the_default_target() {
    assert!(
        rendered().contains("WantedBy=default.target"),
        "got: {}",
        rendered()
    );
}

#[test]
fn the_unit_doubles_percent_signs_in_plain_values() {
    let mut spec = spec_for(UnitKind::Systemd, Path::new("/home/dev"));
    spec.label = "pm3 100% ready".to_string();
    assert!(
        render_unit(&spec).contains("Description=pm3 100%% ready"),
        "got: {}",
        render_unit(&spec)
    );
}

#[test]
fn the_unit_escapes_quotes_and_backslashes_inside_quoted_environment_values() {
    let mut spec = spec_for(UnitKind::Systemd, Path::new("/home/dev"));
    spec.home = "/home/we\"ird\\dev".to_string();
    assert!(
        render_unit(&spec).contains("Environment=\"HOME=/home/we\\\"ird\\\\dev\""),
        "got: {}",
        render_unit(&spec)
    );
}

#[test]
fn the_unit_escapes_quotes_and_backslashes_in_plain_values() {
    let mut spec = spec_for(UnitKind::Systemd, Path::new("/home/dev"));
    spec.working_directory = std::path::PathBuf::from("/opt/we\"ird\\dir");
    assert!(
        render_unit(&spec).contains("WorkingDirectory=/opt/we\\\"ird\\\\dir"),
        "got: {}",
        render_unit(&spec)
    );
}

#[test]
fn the_unit_escapes_quotes_backslashes_and_percent_signs_inside_tokens() {
    let mut spec = spec_for(UnitKind::Systemd, Path::new("/home/dev"));
    spec.program = std::path::PathBuf::from("/opt/a b\\c\"d%e/pm3");
    assert!(
        render_unit(&spec).contains("ExecStart=\"/opt/a b\\\\c\\\"d%%e/pm3\""),
        "got: {}",
        render_unit(&spec)
    );
}

#[test]
fn the_unit_hands_the_daemon_the_pm3_environment_the_install_ran_under() {
    let home = tempfile::tempdir().expect("temp dir");
    let unit = render_unit(&spec_for(UnitKind::Systemd, home.path()));
    assert!(
        unit.contains(&format!(
            "Environment=\"{PM3_HOME_VARIABLE}={PM3_HOME_VALUE}\""
        )),
        "got: {unit}"
    );
}

#[test]
fn the_unit_caps_how_many_tasks_pm3_and_its_services_may_hold() {
    let unit = rendered();
    assert!(
        unit.contains(&format!("TasksMax={MAX_TASKS}")),
        "a fork bomb inside a service must not exhaust the host: {unit}"
    );
}

#[test]
fn the_unit_leaves_the_cpu_unlimited_until_an_operator_asks_for_a_quota() {
    assert!(!rendered().contains("CPUQuota"), "got: {}", rendered());
}

#[test]
fn a_declared_cpu_quota_reaches_the_unit() {
    let mut spec = spec_for(UnitKind::Systemd, Path::new("/home/dev"));
    spec.cpu_quota_percent = 250;
    assert!(
        render_unit(&spec).contains("CPUQuota=250%"),
        "got: {}",
        render_unit(&spec)
    );
}

#[test]
fn a_unit_can_wait_for_the_network() {
    let spec = UnitSpec {
        wait_for_network: true,
        ..spec_for(UnitKind::Systemd, Path::new("/home/dev"))
    };
    let unit = render_unit(&spec);
    assert!(
        unit.contains("Wants=network-online.target\nAfter=network-online.target"),
        "got: {unit}"
    );
}

#[test]
fn a_unit_does_not_wait_for_the_network_by_default() {
    let unit = rendered();
    assert!(!unit.contains("network-online"), "got: {unit}");
}
