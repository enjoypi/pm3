use std::path::Path;

use super::*;
use crate::{
    UnitKind,
    unit_specs::{MAX_TASKS, PM3_HOME_VALUE, PM3_HOME_VARIABLE, spec_for},
};

fn rendered() -> String {
    render_plist(&spec_for(UnitKind::Launchd, Path::new("/home/dev")))
}

#[test]
fn the_plist_opens_with_the_apple_property_list_header() {
    assert!(
        rendered().starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"),
        "got: {}",
        rendered()
    );
}

#[test]
fn the_plist_carries_the_label() {
    assert!(
        rendered().contains("<key>Label</key>\n    <string>pm3-test</string>"),
        "got: {}",
        rendered()
    );
}

#[test]
fn the_plist_spells_out_the_daemon_invocation() {
    let expected = "        <string>/usr/local/bin/pm3</string>\n        <string>daemon</string>\n        <string>--config</string>\n        <string>/home/dev/.pm3/config.yaml</string>\n";
    assert!(rendered().contains(expected), "got: {}", rendered());
}

#[test]
fn the_plist_asks_launchd_to_start_and_keep_the_daemon_alive() {
    let plist = rendered();
    assert!(
        plist.contains("<key>RunAtLoad</key>\n    <true/>"),
        "got: {plist}"
    );
    assert!(
        plist.contains("<key>KeepAlive</key>\n    <true/>"),
        "got: {plist}"
    );
}

#[test]
fn a_launch_agent_keeps_alive_only_after_a_failure_when_the_config_says_so() {
    let mut spec = spec_for(UnitKind::Launchd, Path::new("/home/dev"));
    spec.restart_condition = "on-failure".to_string();
    let plist = render_plist(&spec);
    assert!(
        plist.contains("<key>SuccessfulExit</key>\n        <false/>"),
        "got: {plist}"
    );
}

#[test]
fn the_plist_tells_launchd_to_leave_the_service_process_group_alone() {
    assert!(
        rendered().contains("<key>AbandonProcessGroup</key>\n    <true/>"),
        "got: {}",
        rendered()
    );
}

#[test]
fn the_plist_carries_the_configured_restart_delay() {
    assert!(
        rendered().contains("<key>ThrottleInterval</key>\n    <integer>2</integer>"),
        "launchd otherwise throttles restarts at its own 10 second default: {}",
        rendered()
    );
}

#[test]
fn the_plist_takes_the_restart_delay_from_the_spec() {
    let mut spec = spec_for(UnitKind::Launchd, Path::new("/home/dev"));
    spec.restart_delay_secs = 9;
    assert!(
        render_plist(&spec).contains("<key>ThrottleInterval</key>\n    <integer>9</integer>"),
        "got: {}",
        render_plist(&spec)
    );
}

#[test]
fn the_plist_keeps_every_file_the_daemon_writes_to_its_owner() {
    assert!(
        rendered().contains("<key>Umask</key>\n    <integer>63</integer>"),
        "got: {}",
        rendered()
    );
}

#[test]
fn the_plist_points_both_streams_at_the_daemon_log() {
    let plist = rendered();
    assert!(
        plist.contains("<key>StandardOutPath</key>\n    <string>/home/dev/.pm3/pm3.log</string>"),
        "got: {plist}"
    );
    assert!(
        plist.contains("<key>StandardErrorPath</key>\n    <string>/home/dev/.pm3/pm3.log</string>"),
        "got: {plist}"
    );
}

#[test]
fn the_plist_forwards_the_search_path() {
    assert!(
        rendered().contains("<key>PATH</key>\n        <string>/usr/bin:/bin</string>"),
        "got: {}",
        rendered()
    );
}

#[test]
fn the_plist_escapes_every_xml_special_character() {
    let mut spec = spec_for(UnitKind::Launchd, Path::new("/home/dev"));
    spec.label = "a&b<c>d\"e'f".to_string();
    let plist = render_plist(&spec);
    assert!(
        plist.contains("<string>a&amp;b&lt;c&gt;d&quot;e&apos;f</string>"),
        "got: {plist}"
    );
}

#[test]
fn the_plist_hands_the_daemon_the_pm3_environment_the_install_ran_under() {
    let home = tempfile::tempdir().expect("temp dir");
    let plist = render_plist(&spec_for(UnitKind::Launchd, home.path()));
    assert!(
        plist.contains(&format!(
            "<key>{PM3_HOME_VARIABLE}</key>\n        <string>{PM3_HOME_VALUE}</string>"
        )),
        "got: {plist}"
    );
}

#[test]
fn the_plist_caps_how_many_processes_pm3_and_its_services_may_hold() {
    let home = tempfile::tempdir().expect("temp dir");
    let plist = render_plist(&spec_for(UnitKind::Launchd, home.path()));
    assert!(
        plist.contains(&format!(
            "<key>NumberOfProcesses</key>\n        <integer>{MAX_TASKS}</integer>"
        )),
        "a fork bomb inside a service must not exhaust the host: {plist}"
    );
}

#[test]
fn the_plist_carries_no_cpu_quota_because_launchd_offers_none() {
    let home = tempfile::tempdir().expect("temp dir");
    let mut spec = spec_for(UnitKind::Launchd, home.path());
    spec.cpu_quota_percent = 250;
    assert!(
        !render_plist(&spec).contains("CPU"),
        "launchd only caps total cpu seconds, which would kill a healthy long-running daemon"
    );
}

#[test]
fn a_launchd_unit_never_waits_for_the_network() {
    let spec = UnitSpec {
        wait_for_network: true,
        ..spec_for(UnitKind::Launchd, Path::new("/home/dev"))
    };
    let plist = render_plist(&spec);
    assert!(!plist.contains("network-online"), "got: {plist}");
}
