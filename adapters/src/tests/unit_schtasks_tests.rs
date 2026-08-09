use std::path::Path;

use super::*;
use crate::{UnitKind, unit_specs::spec_for};

#[test]
fn the_task_registers_a_logon_trigger_for_the_wrapper() {
    let xml = render_task_xml(&spec_for(UnitKind::WinSchtasks, Path::new("/home/dev")));
    assert!(xml.contains("<LogonTrigger>"));
    assert!(xml.contains("<Description>pm3-test</Description>"));
    assert!(xml.contains("<Command>/home/dev/.pm3/service/pm3-test-daemon.cmd</Command>"));
    assert!(xml.contains("<WorkingDirectory>/home/dev/.pm3</WorkingDirectory>"));
}

#[test]
fn the_task_runs_as_the_interactive_user_with_least_privilege() {
    let xml = render_task_xml(&spec_for(UnitKind::WinSchtasks, Path::new("/home/dev")));
    assert!(xml.contains("<LogonType>InteractiveToken</LogonType>"));
    assert!(xml.contains("<RunLevel>LeastPrivilege</RunLevel>"));
}

#[test]
fn the_task_never_times_out_and_ignores_duplicate_triggers() {
    let xml = render_task_xml(&spec_for(UnitKind::WinSchtasks, Path::new("/home/dev")));
    assert!(xml.contains("<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>"));
    assert!(xml.contains("<MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>"));
}

#[test]
fn the_task_restarts_on_failure_with_a_floor_of_one_minute() {
    let xml = render_task_xml(&spec_for(UnitKind::WinSchtasks, Path::new("/home/dev")));
    assert!(xml.contains("<Interval>PT60S</Interval>"));
    assert!(xml.contains("<Count>999</Count>"));
}

#[test]
fn a_restart_delay_above_the_floor_is_kept() {
    let mut spec = spec_for(UnitKind::WinSchtasks, Path::new("/home/dev"));
    spec.restart_delay_secs = 90;
    let xml = render_task_xml(&spec);
    assert!(xml.contains("<Interval>PT90S</Interval>"));
}

#[test]
fn the_task_runs_on_battery_and_survives_a_power_switch() {
    let xml = render_task_xml(&spec_for(UnitKind::WinSchtasks, Path::new("/home/dev")));
    assert!(xml.contains("<DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>"));
    assert!(xml.contains("<StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>"));
}

#[test]
fn markup_in_paths_is_escaped() {
    let mut spec = spec_for(UnitKind::WinSchtasks, Path::new("/home/dev"));
    spec.label = "a&b<c>".to_string();
    let xml = render_task_xml(&spec);
    assert!(xml.contains("<Description>a&amp;b&lt;c&gt;</Description>"));
    assert!(!xml.contains("a&b<c>"));
}

#[test]
fn unix_only_limits_are_not_rendered() {
    let xml = render_task_xml(&spec_for(UnitKind::WinSchtasks, Path::new("/home/dev")));
    assert!(
        !xml.contains("4096"),
        "TasksMax has no Task Scheduler equivalent"
    );
    assert!(!xml.contains("CPUQuota"));
    assert!(
        !xml.contains("0077"),
        "umask has no Task Scheduler equivalent"
    );
    assert!(!xml.contains("network-online"));
}

#[test]
fn the_wrapper_exports_home_path_and_the_sorted_pm3_environment() {
    let wrapper = render_wrapper(&spec_for(UnitKind::WinSchtasks, Path::new("/home/dev")));
    assert!(wrapper.starts_with("@echo off\r\n"));
    assert!(wrapper.contains("set \"HOME=/home/dev\"\r\n"));
    assert!(wrapper.contains("set \"PATH=/usr/bin:/bin\"\r\n"));
    assert!(wrapper.contains("set \"PM3_HOME=/srv/pm3\"\r\n"));
}

#[test]
fn the_wrapper_runs_the_daemon_and_always_reports_failure() {
    let wrapper = render_wrapper(&spec_for(UnitKind::WinSchtasks, Path::new("/home/dev")));
    assert!(wrapper.contains(
        "\"/usr/local/bin/pm3\" daemon --config \"/home/dev/.pm3/config.yaml\" >> \"/home/dev/.pm3/pm3.log\" 2>&1\r\n"
    ));
    assert!(wrapper.ends_with("exit /b 1\r\n"));
}

#[test]
fn percent_signs_in_values_are_doubled_for_batch_files() {
    let mut spec = spec_for(UnitKind::WinSchtasks, Path::new("/home/dev"));
    spec.search_path = "C:\\100%.bin".to_string();
    let wrapper = render_wrapper(&spec);
    assert!(wrapper.contains("set \"PATH=C:\\100%%.bin\"\r\n"));
}
