use usecases::{ReadScope, SandboxMode, SandboxPolicy};

use super::*;
use crate::process_records::sandbox_policy;

const PROGRAM: &str = "/usr/bin/node";

fn listening(ports: &[u16]) -> String {
    let policy = SandboxPolicy {
        listen_ports: ports.to_vec(),
        ..sandbox_policy(SandboxMode::WorkspaceWrite, ReadScope::Minimal, false, &[])
    };
    seatbelt_profile(&policy, PROGRAM).profile
}

#[test]
fn a_listen_port_allows_only_inbound_tcp_on_that_port() {
    let profile = listening(&[8089]);
    assert!(
        profile.contains("(allow network-inbound (local tcp \"localhost:8089\"))"),
        "got: {profile}"
    );
}

#[test]
fn every_listen_port_gets_its_own_rule() {
    let profile = listening(&[8089, 9000]);
    assert!(profile.contains("\"localhost:8089\""), "got: {profile}");
    assert!(profile.contains("\"localhost:9000\""), "got: {profile}");
}

#[test]
fn a_listen_port_opens_nothing_outbound() {
    let profile = listening(&[8089]);
    for refused in ["network-outbound", "mDNSResponder", "system-socket"] {
        assert!(!profile.contains(refused), "{refused} leaked: {profile}");
    }
}

#[test]
fn an_app_without_listen_ports_has_no_network_rule() {
    let profile = listening(&[]);
    assert!(!profile.contains("network-"), "got: {profile}");
}
