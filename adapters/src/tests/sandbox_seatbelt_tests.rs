use usecases::{ReadScope, SandboxMode, SandboxPolicy};

use super::*;

const SEATBELT_PROGRAM: &str = "/usr/bin/sandbox-exec";
const PROGRAM: &str = "/usr/bin/node";

fn policy(mode: SandboxMode, network: bool, writable_roots: &[&str]) -> SandboxPolicy {
    SandboxPolicy {
        mode,
        read: ReadScope::Full,
        network,
        writable_roots: writable_roots.iter().map(|r| (*r).to_string()).collect(),
        readable_roots: Vec::new(),
        derived_roots: Vec::new(),
        unreadable_roots: Vec::new(),
    }
}

fn profile_of(policy: &SandboxPolicy) -> String {
    seatbelt_profile(policy, PROGRAM).profile
}

fn parameters_of(policy: &SandboxPolicy) -> Vec<(String, String)> {
    seatbelt_profile(policy, PROGRAM).parameters
}

#[test]
fn the_profile_denies_everything_by_default() {
    let profile = profile_of(&policy(SandboxMode::ReadOnly, false, &[]));
    assert!(profile.contains("(deny default)"), "got: {profile}");
}

#[test]
fn a_full_read_scope_allows_reading_the_whole_disk() {
    let profile = profile_of(&policy(SandboxMode::ReadOnly, false, &[]));
    assert!(profile.contains("(allow file-read*)"), "got: {profile}");
}

#[test]
fn a_minimal_read_scope_replaces_the_blanket_read_rule_with_an_allowlist() {
    let confined = SandboxPolicy {
        read: ReadScope::Minimal,
        ..policy(SandboxMode::WorkspaceWrite, false, &[])
    };
    let profile = profile_of(&confined);
    assert!(
        !profile.contains("(allow file-read*)\n"),
        "the blanket rule must be gone: {profile}"
    );
    assert!(
        profile.contains("(subpath \"/usr/lib\")"),
        "the system allowlist must be in: {profile}"
    );
}

#[test]
fn a_minimal_read_scope_keeps_the_program_itself_readable() {
    let confined = SandboxPolicy {
        read: ReadScope::Minimal,
        ..policy(SandboxMode::WorkspaceWrite, false, &[])
    };
    let parameters = parameters_of(&confined);
    assert!(
        parameters.contains(&("READABLE_0".to_string(), PROGRAM.to_string())),
        "got: {parameters:?}"
    );
}

#[test]
fn a_minimal_read_scope_carries_the_declared_readable_roots() {
    let confined = SandboxPolicy {
        read: ReadScope::Minimal,
        readable_roots: vec!["/opt/data/".to_string()],
        ..policy(SandboxMode::WorkspaceWrite, false, &[])
    };
    let parameters = parameters_of(&confined);
    assert_eq!(
        parameters.first(),
        Some(&("READABLE_0".to_string(), "/opt/data".to_string()))
    );
}

#[test]
fn a_full_read_scope_declares_no_readable_parameters() {
    let parameters = parameters_of(&policy(SandboxMode::WorkspaceWrite, false, &[]));
    assert!(
        !parameters.iter().any(|(name, _)| name.starts_with("READ")),
        "got: {parameters:?}"
    );
}

#[test]
fn a_confined_profile_carries_no_network_rules() {
    let profile = profile_of(&policy(SandboxMode::WorkspaceWrite, false, &[]));
    assert!(
        !profile.contains("network-outbound"),
        "network must stay closed: {profile}"
    );
}

#[test]
fn enabling_the_network_appends_the_network_rules() {
    let profile = profile_of(&policy(SandboxMode::WorkspaceWrite, true, &[]));
    assert!(
        profile.contains("(allow network-outbound (remote ip))"),
        "got: {profile}"
    );
    assert!(
        profile.contains("com.apple.SystemConfiguration.DNSConfiguration"),
        "dns lookups must be permitted: {profile}"
    );
}

#[test]
fn the_network_rules_allow_the_system_resolver() {
    let profile = profile_of(&policy(SandboxMode::WorkspaceWrite, true, &[]));
    assert!(
        profile.contains("com.apple.mDNSResponder"),
        "some clients reach mDNSResponder over mach: {profile}"
    );
    assert!(
        profile.contains("(allow network-outbound (literal \"/private/var/run/mDNSResponder\"))"),
        "getaddrinfo resolves through the mDNSResponder unix socket and must be reachable: {profile}"
    );
    assert!(
        profile.contains("(literal \"/private/var/run/resolv.conf\")"),
        "clients read the system DNS config from resolv.conf and must be able to: {profile}"
    );
}

#[test]
fn enabling_the_network_still_refuses_unix_sockets() {
    let profile = profile_of(&policy(SandboxMode::WorkspaceWrite, true, &[]));
    assert!(
        !profile.contains("(allow network-outbound)\n"),
        "an unqualified rule would reach the pm3 control socket: {profile}"
    );
    assert!(
        !profile.contains("(allow network-outbound (remote unix-socket))"),
        "unix sockets must stay closed except for the mDNSResponder literal: {profile}"
    );
}

#[test]
fn each_writable_root_becomes_a_parameterised_subpath_rule() {
    let confined = policy(
        SandboxMode::WorkspaceWrite,
        false,
        &["/srv/api", "/var/cache/api"],
    );
    let profile = profile_of(&confined);
    assert!(
        profile.contains("file-write* (require-all (subpath (param \"WRITABLE_0\")))"),
        "got: {profile}"
    );
    assert!(
        profile.contains("file-write* (require-all (subpath (param \"WRITABLE_1\")))"),
        "got: {profile}"
    );
    assert_eq!(
        parameters_of(&confined),
        vec![
            ("WRITABLE_0".to_string(), "/srv/api".to_string()),
            ("WRITABLE_1".to_string(), "/var/cache/api".to_string()),
        ]
    );
}

#[test]
fn no_path_is_ever_interpolated_into_the_profile_text() {
    let confined = policy(SandboxMode::WorkspaceWrite, false, &["/srv/api"]);
    let profile = profile_of(&confined);
    assert!(
        !profile.contains("/srv/api"),
        "paths belong in -D parameters, not in the policy text: {profile}"
    );
}

#[test]
fn a_writable_root_keeps_no_trailing_slash() {
    let parameters = parameters_of(&policy(SandboxMode::WorkspaceWrite, false, &["/srv/api/"]));
    assert_eq!(
        parameters.first(),
        Some(&("WRITABLE_0".to_string(), "/srv/api".to_string()))
    );
}

#[test]
fn the_filesystem_root_stays_a_root_when_writable() {
    let parameters = parameters_of(&policy(SandboxMode::WorkspaceWrite, false, &["/"]));
    assert_eq!(
        parameters.first(),
        Some(&("WRITABLE_0".to_string(), "/".to_string()))
    );
}

fn hidden_policy() -> SandboxPolicy {
    SandboxPolicy {
        derived_roots: vec!["/home/me/.pm3/api".to_string()],
        unreadable_roots: vec!["/home/me/.pm3".to_string()],
        ..policy(SandboxMode::WorkspaceWrite, false, &[])
    }
}

#[test]
fn a_workspace_inside_a_hidden_root_keeps_its_writable_rule() {
    let profile = profile_of(&hidden_policy());
    assert!(
        profile.contains("(subpath (param \"WRITABLE_0\"))"),
        "the workspace grant must exist: {profile}"
    );
    assert!(
        !profile.contains("(param \"WRITABLE_0\")) (require-not"),
        "an ancestor hidden root must not void the nested workspace grant: {profile}"
    );
}

#[test]
fn a_hidden_root_inside_a_granted_root_is_carved_out() {
    let nested = SandboxPolicy {
        writable_roots: vec!["/srv".to_string()],
        unreadable_roots: vec!["/srv/secrets".to_string()],
        ..policy(SandboxMode::WorkspaceWrite, false, &[])
    };
    let profile = profile_of(&nested);
    assert!(
        profile.contains(
            "(subpath (param \"WRITABLE_0\")) (require-not (subpath (param \"HIDDEN_0\")))"
        ),
        "a granted root must not expose a hidden root nested inside it: {profile}"
    );
}

#[test]
fn a_readable_root_inside_a_hidden_root_keeps_its_read_rule() {
    let confined = SandboxPolicy {
        read: ReadScope::Minimal,
        readable_roots: vec!["/home/me/.pm3/api".to_string()],
        unreadable_roots: vec!["/home/me/.pm3".to_string()],
        ..policy(SandboxMode::WorkspaceWrite, false, &[])
    };
    let profile = profile_of(&confined);
    assert!(
        !profile.contains("(param \"READABLE_0\")) (require-not"),
        "an ancestor hidden root must not void the nested readable grant: {profile}"
    );
}

#[test]
fn a_granted_root_equal_to_a_hidden_root_stays_carved_out() {
    let confined = SandboxPolicy {
        read: ReadScope::Minimal,
        readable_roots: vec!["/home/me/.pm3".to_string()],
        unreadable_roots: vec!["/home/me/.pm3".to_string()],
        ..policy(SandboxMode::WorkspaceWrite, false, &[])
    };
    let profile = profile_of(&confined);
    assert!(
        profile.contains(
            "(subpath (param \"READABLE_0\")) (require-not (subpath (param \"HIDDEN_0\")))"
        ),
        "declaring the pm3 home itself readable must not open it: {profile}"
    );
}

#[test]
fn a_hidden_root_is_carved_out_of_the_whole_disk_read_rule() {
    let profile = profile_of(&hidden_policy());
    assert!(
        !profile.contains("(allow file-read*)\n"),
        "the blanket read rule must carry the carveout: {profile}"
    );
    assert!(
        profile.contains("(subpath \"/\") (require-not (subpath (param \"HIDDEN_0\")))"),
        "got: {profile}"
    );
}

#[test]
fn a_hidden_root_travels_as_a_parameter() {
    assert!(
        parameters_of(&hidden_policy())
            .contains(&("HIDDEN_0".to_string(), "/home/me/.pm3".to_string())),
        "got: {:?}",
        parameters_of(&hidden_policy())
    );
}

#[test]
fn the_command_is_handed_to_sandbox_exec_after_a_separator() {
    let wrapped = seatbelt_argv(
        SEATBELT_PROGRAM,
        &policy(SandboxMode::WorkspaceWrite, false, &[]),
        PROGRAM,
        &["server.js".to_string()],
    );
    assert_eq!(wrapped.program, "/usr/bin/sandbox-exec");
    assert_eq!(wrapped.args.first().map(String::as_str), Some("-p"));
    assert_eq!(
        wrapped.args.get(2).map(String::as_str),
        Some("--"),
        "the profile must be followed by a separator"
    );
    assert_eq!(wrapped.args.get(3).map(String::as_str), Some(PROGRAM));
    assert_eq!(wrapped.args.get(4).map(String::as_str), Some("server.js"));
}

#[test]
fn every_root_travels_as_a_command_line_parameter() {
    let wrapped = seatbelt_argv(
        SEATBELT_PROGRAM,
        &policy(SandboxMode::WorkspaceWrite, false, &["/srv/api"]),
        PROGRAM,
        &[],
    );
    assert_eq!(wrapped.args.first().map(String::as_str), Some("-D"));
    assert_eq!(
        wrapped.args.get(1).map(String::as_str),
        Some("WRITABLE_0=/srv/api")
    );
}

#[test]
fn the_profile_follows_the_parameters() {
    let wrapped = seatbelt_argv(
        SEATBELT_PROGRAM,
        &policy(SandboxMode::WorkspaceWrite, false, &[]),
        PROGRAM,
        &[],
    );
    let profile = wrapped.args.get(1).expect("profile argument present");
    assert!(profile.contains("(deny default)"), "got: {profile}");
}
