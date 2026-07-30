use usecases::{SandboxMode, SandboxPolicy};

use super::*;

fn policy(mode: SandboxMode, network: bool, writable_roots: &[&str]) -> SandboxPolicy {
    SandboxPolicy {
        mode,
        network,
        writable_roots: writable_roots.iter().map(|r| (*r).to_string()).collect(),
    }
}

#[test]
fn the_profile_denies_everything_by_default() {
    let profile = seatbelt_profile(&policy(SandboxMode::ReadOnly, false, &[]));
    assert!(profile.contains("(deny default)"), "got: {profile}");
}

#[test]
fn the_profile_allows_reading_the_whole_disk() {
    let profile = seatbelt_profile(&policy(SandboxMode::ReadOnly, false, &[]));
    assert!(profile.contains("(allow file-read*)"), "got: {profile}");
}

#[test]
fn a_confined_profile_carries_no_network_rules() {
    let profile = seatbelt_profile(&policy(SandboxMode::WorkspaceWrite, false, &[]));
    assert!(
        !profile.contains("network-outbound"),
        "network must stay closed: {profile}"
    );
}

#[test]
fn enabling_the_network_appends_the_network_rules() {
    let profile = seatbelt_profile(&policy(SandboxMode::WorkspaceWrite, true, &[]));
    assert!(
        profile.contains("(allow network-outbound)"),
        "got: {profile}"
    );
    assert!(
        profile.contains("com.apple.SystemConfiguration.DNSConfiguration"),
        "dns lookups must be permitted: {profile}"
    );
}

#[test]
fn each_writable_root_becomes_a_subpath_rule() {
    let profile = seatbelt_profile(&policy(
        SandboxMode::WorkspaceWrite,
        false,
        &["/srv/api", "/var/cache/api"],
    ));
    assert!(
        profile.contains("(allow file-write* (subpath \"/srv/api\"))"),
        "got: {profile}"
    );
    assert!(
        profile.contains("(allow file-write* (subpath \"/var/cache/api\"))"),
        "got: {profile}"
    );
}

#[test]
fn a_writable_root_keeps_no_trailing_slash() {
    let profile = seatbelt_profile(&policy(SandboxMode::WorkspaceWrite, false, &["/srv/api/"]));
    assert!(profile.contains("(subpath \"/srv/api\")"), "got: {profile}");
}

#[test]
fn the_command_is_handed_to_sandbox_exec_after_a_separator() {
    let wrapped = seatbelt_argv(
        &policy(SandboxMode::WorkspaceWrite, false, &[]),
        "/usr/bin/node",
        &["server.js".to_string()],
    );
    assert_eq!(wrapped.program, "/usr/bin/sandbox-exec");
    assert_eq!(wrapped.args.first().map(String::as_str), Some("-p"));
    assert_eq!(
        wrapped.args.get(2).map(String::as_str),
        Some("--"),
        "the profile must be followed by a separator"
    );
    assert_eq!(
        wrapped.args.get(3).map(String::as_str),
        Some("/usr/bin/node")
    );
    assert_eq!(wrapped.args.get(4).map(String::as_str), Some("server.js"));
}

#[test]
fn the_profile_is_passed_inline_as_the_second_argument() {
    let wrapped = seatbelt_argv(
        &policy(SandboxMode::WorkspaceWrite, false, &[]),
        "/usr/bin/node",
        &[],
    );
    let profile = wrapped.args.get(1).expect("profile argument present");
    assert!(profile.contains("(deny default)"), "got: {profile}");
}
