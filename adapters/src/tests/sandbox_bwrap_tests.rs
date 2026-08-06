use usecases::{ReadScope, SandboxMode, SandboxPolicy};

use super::*;

const BWRAP_PROGRAM: &str = "bwrap";
const PROGRAM: &str = "/usr/bin/node";

fn minimal_roots() -> Vec<String> {
    vec!["/usr".to_string(), "/etc".to_string()]
}

fn policy(network: bool, writable_roots: &[&str]) -> SandboxPolicy {
    SandboxPolicy {
        mode: SandboxMode::WorkspaceWrite,
        read: ReadScope::Full,
        network,
        writable_roots: writable_roots.iter().map(|r| (*r).to_string()).collect(),
        readable_roots: Vec::new(),
        derived_roots: Vec::new(),
        unreadable_roots: Vec::new(),
    }
}

fn argv_for(policy: &SandboxPolicy) -> Vec<String> {
    bwrap_argv(
        BWRAP_PROGRAM,
        &minimal_roots(),
        policy,
        PROGRAM,
        &["server.js".to_string()],
    )
    .args
}

fn argv(network: bool, writable_roots: &[&str]) -> Vec<String> {
    argv_for(&policy(network, writable_roots))
}

fn index_of(args: &[String], needle: &str) -> usize {
    args.iter()
        .position(|arg| arg == needle)
        .unwrap_or_else(|| panic!("{needle} is required, got: {args:?}"))
}

fn triple_at(args: &[String], flag: &str) -> (String, String) {
    let index = index_of(args, flag);
    (
        args.get(index + 1).cloned().unwrap_or_default(),
        args.get(index + 2).cloned().unwrap_or_default(),
    )
}

#[test]
fn the_sandbox_outlives_its_parent_so_a_daemon_restart_can_reclaim_it() {
    assert!(
        !argv(false, &[]).contains(&"--die-with-parent".to_string()),
        "a confined service must survive the daemon it was launched from"
    );
}

#[test]
fn the_sandbox_keeps_the_session_of_its_launcher_so_group_signals_still_reach_it() {
    assert!(
        !argv(false, &[]).contains(&"--new-session".to_string()),
        "setsid would detach the service from the process group pm3 signals"
    );
}

#[test]
fn the_user_and_pid_namespaces_are_always_unshared() {
    let args = argv(false, &[]);
    assert!(args.contains(&"--unshare-user".to_string()));
    assert!(args.contains(&"--unshare-pid".to_string()));
}

#[test]
fn the_ipc_and_uts_namespaces_are_always_unshared() {
    let args = argv(false, &[]);
    assert!(args.contains(&"--unshare-ipc".to_string()));
    assert!(args.contains(&"--unshare-uts".to_string()));
}

#[test]
fn the_cgroup_namespace_is_unshared_where_the_kernel_offers_it() {
    assert!(
        argv(false, &[]).contains(&"--unshare-cgroup-try".to_string()),
        "an older kernel without cgroup namespaces must still start the service"
    );
}

#[test]
fn a_full_read_scope_mounts_the_whole_filesystem_read_only() {
    let args = argv(false, &[]);
    assert_eq!(
        triple_at(&args, "--ro-bind"),
        ("/".to_string(), "/".to_string())
    );
}

#[test]
fn a_minimal_read_scope_starts_from_an_empty_root() {
    let confined = SandboxPolicy {
        read: ReadScope::Minimal,
        ..policy(false, &[])
    };
    let args = argv_for(&confined);
    let tmpfs = index_of(&args, "--tmpfs");
    assert_eq!(args.get(tmpfs + 1).map(String::as_str), Some("/"));
    assert!(
        !args.contains(&"--ro-bind".to_string()),
        "nothing may be bound read-only outside the allowlist: {args:?}"
    );
}

#[test]
fn a_minimal_read_scope_lays_the_configured_system_roots_back_in() {
    let confined = SandboxPolicy {
        read: ReadScope::Minimal,
        ..policy(false, &[])
    };
    let args = argv_for(&confined);
    assert_eq!(
        triple_at(&args, "--ro-bind-try"),
        ("/usr".to_string(), "/usr".to_string())
    );
    assert!(args.contains(&"/etc".to_string()), "got: {args:?}");
}

#[test]
fn a_minimal_read_scope_lays_the_declared_readable_roots_back_in() {
    let confined = SandboxPolicy {
        read: ReadScope::Minimal,
        readable_roots: vec!["/opt/data/".to_string()],
        ..policy(false, &[])
    };
    let args = argv_for(&confined);
    assert!(args.contains(&"/opt/data".to_string()), "got: {args:?}");
}

#[test]
fn a_minimal_read_scope_keeps_the_program_itself_readable() {
    let confined = SandboxPolicy {
        read: ReadScope::Minimal,
        ..policy(false, &[])
    };
    let args = argv_for(&confined);
    assert!(
        args.iter().filter(|arg| *arg == PROGRAM).count() >= 2,
        "the binary must be bound in or exec fails: {args:?}"
    );
}

#[test]
fn a_full_read_scope_does_not_lay_any_allowlist_back_in() {
    assert!(
        !argv(false, &[]).contains(&"--ro-bind-try".to_string()),
        "a full read scope needs no allowlist"
    );
}

#[test]
fn a_confined_app_loses_its_network_namespace() {
    assert!(argv(false, &[]).contains(&"--unshare-net".to_string()));
}

#[test]
fn an_app_with_network_access_keeps_its_network_namespace() {
    assert!(!argv(true, &[]).contains(&"--unshare-net".to_string()));
}

#[test]
fn each_writable_root_is_bound_read_write() {
    let args = argv(false, &["/srv/api"]);
    assert_eq!(
        triple_at(&args, "--bind"),
        ("/srv/api".to_string(), "/srv/api".to_string())
    );
}

#[test]
fn a_writable_root_keeps_no_trailing_slash() {
    let args = argv(false, &["/srv/api/"]);
    assert!(args.contains(&"/srv/api".to_string()), "got: {args:?}");
}

#[test]
fn the_filesystem_root_stays_a_root_when_writable() {
    let args = argv(false, &["/"]);
    assert_eq!(
        triple_at(&args, "--bind"),
        ("/".to_string(), "/".to_string())
    );
}

#[test]
fn a_hidden_root_is_masked_by_a_tmpfs() {
    let hidden = SandboxPolicy {
        unreadable_roots: vec!["/home/me/.config/pm3/".to_string()],
        ..policy(false, &[])
    };
    let args = argv_for(&hidden);
    let tmpfs = index_of(&args, "--tmpfs");
    assert_eq!(
        args.get(tmpfs + 1).map(String::as_str),
        Some("/home/me/.config/pm3")
    );
}

#[test]
fn a_hidden_root_is_masked_before_the_writable_roots_are_bound_back_in() {
    let hidden = SandboxPolicy {
        derived_roots: vec!["/home/me/.pm3/api".to_string()],
        unreadable_roots: vec!["/home/me/.pm3".to_string()],
        ..policy(false, &[])
    };
    let args = argv_for(&hidden);
    assert!(
        index_of(&args, "--tmpfs") < index_of(&args, "--bind"),
        "the workspace must be restored after the mask, got: {args:?}"
    );
}

#[test]
fn writable_roots_are_bound_shallowest_first() {
    let nested = SandboxPolicy {
        writable_roots: vec!["/srv/api/data".to_string(), "/srv".to_string()],
        ..policy(false, &[])
    };
    let args = argv_for(&nested);
    let shallow = index_of(&args, "/srv");
    let deep = index_of(&args, "/srv/api/data");
    assert!(shallow < deep, "got: {args:?}");
}

#[test]
fn the_command_follows_a_separator() {
    let args = argv(false, &[]);
    let index = index_of(&args, "--");
    assert_eq!(args.get(index + 1).map(String::as_str), Some(PROGRAM));
    assert_eq!(args.get(index + 2).map(String::as_str), Some("server.js"));
}

#[test]
fn bubblewrap_is_looked_up_on_the_path() {
    let wrapped = bwrap_argv(
        BWRAP_PROGRAM,
        &minimal_roots(),
        &policy(false, &[]),
        PROGRAM,
        &[],
    );
    assert_eq!(wrapped.program, "bwrap");
}

#[test]
fn a_hidden_root_nested_in_a_writable_root_is_masked_again_after_the_bind() {
    let nested = SandboxPolicy {
        writable_roots: vec!["/home/me".to_string()],
        unreadable_roots: vec!["/home/me/.config/pm3".to_string()],
        ..policy(false, &[])
    };
    let args = argv_for(&nested);
    let last = args
        .iter()
        .rposition(|arg| arg == "--tmpfs")
        .expect("a mask is required");
    assert_eq!(
        args.get(last + 1).map(String::as_str),
        Some("/home/me/.config/pm3")
    );
    assert!(
        last > index_of(&args, "--bind"),
        "the mask must survive the bind that would reopen it: {args:?}"
    );
}

#[test]
fn a_hidden_root_that_is_itself_a_writable_root_is_not_masked_twice() {
    let same = SandboxPolicy {
        writable_roots: vec!["/home/me/.pm3".to_string()],
        unreadable_roots: vec!["/home/me/.pm3".to_string()],
        ..policy(false, &[])
    };
    let args = argv_for(&same);
    assert!(
        args.iter().rposition(|arg| arg == "--tmpfs") < Some(index_of(&args, "--bind")),
        "masking it again would take the workspace with it: {args:?}"
    );
}
