use std::path::Path;

use super::*;
use crate::{ServiceKind, service_specs::spec_for};

fn rendered() -> String {
    render_plist(&spec_for(ServiceKind::Launchd, Path::new("/home/dev")))
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
    let mut spec = spec_for(ServiceKind::Launchd, Path::new("/home/dev"));
    spec.label = "a&b<c>d\"e'f".to_string();
    let plist = render_plist(&spec);
    assert!(
        plist.contains("<string>a&amp;b&lt;c&gt;d&quot;e&apos;f</string>"),
        "got: {plist}"
    );
}
