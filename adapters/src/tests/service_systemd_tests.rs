use std::path::Path;

use super::*;
use crate::{ServiceKind, service_specs::spec_for};

fn rendered() -> String {
    render_unit(&spec_for(ServiceKind::Systemd, Path::new("/home/dev")))
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
fn the_unit_restarts_the_daemon_on_failure() {
    let unit = rendered();
    assert!(unit.contains("Restart=on-failure"), "got: {unit}");
    assert!(unit.contains("RestartSec=2"), "got: {unit}");
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
    let mut spec = spec_for(ServiceKind::Systemd, Path::new("/home/dev"));
    spec.label = "pm3 100% ready".to_string();
    assert!(
        render_unit(&spec).contains("Description=pm3 100%% ready"),
        "got: {}",
        render_unit(&spec)
    );
}

#[test]
fn the_unit_escapes_quotes_backslashes_and_percent_signs_inside_tokens() {
    let mut spec = spec_for(ServiceKind::Systemd, Path::new("/home/dev"));
    spec.program = std::path::PathBuf::from("/opt/a b\\c\"d%e/pm3");
    assert!(
        render_unit(&spec).contains("ExecStart=\"/opt/a b\\\\c\\\"d%%e/pm3\""),
        "got: {}",
        render_unit(&spec)
    );
}
