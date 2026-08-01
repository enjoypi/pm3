use usecases::{SandboxMode, SandboxPolicy};

use super::{super::backend::BWRAP_PROGRAM, *};

fn policy(network: bool, writable_roots: &[&str]) -> SandboxPolicy {
    SandboxPolicy {
        mode: SandboxMode::WorkspaceWrite,
        network,
        writable_roots: writable_roots.iter().map(|r| (*r).to_string()).collect(),
        derived_roots: Vec::new(),
    }
}

fn argv(network: bool, writable_roots: &[&str]) -> Vec<String> {
    bwrap_argv(
        BWRAP_PROGRAM,
        &policy(network, writable_roots),
        "/usr/bin/node",
        &["server.js".to_string()],
    )
    .args
}

#[test]
fn the_sandbox_outlives_its_parent_so_a_daemon_restart_can_reclaim_it() {
    assert!(
        !argv(false, &[]).contains(&"--die-with-parent".to_string()),
        "a confined service must survive the daemon it was launched from"
    );
}

#[test]
fn the_user_and_pid_namespaces_are_always_unshared() {
    let args = argv(false, &[]);
    assert!(args.contains(&"--unshare-user".to_string()));
    assert!(args.contains(&"--unshare-pid".to_string()));
}

#[test]
fn the_whole_filesystem_is_mounted_read_only() {
    let args = argv(false, &[]);
    let index = args
        .iter()
        .position(|arg| arg == "--ro-bind")
        .expect("a read-only root bind is required");
    assert_eq!(args.get(index + 1).map(String::as_str), Some("/"));
    assert_eq!(args.get(index + 2).map(String::as_str), Some("/"));
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
    let index = args
        .iter()
        .position(|arg| arg == "--bind")
        .expect("a writable bind is required");
    assert_eq!(args.get(index + 1).map(String::as_str), Some("/srv/api"));
    assert_eq!(args.get(index + 2).map(String::as_str), Some("/srv/api"));
}

#[test]
fn a_writable_root_keeps_no_trailing_slash() {
    let args = argv(false, &["/srv/api/"]);
    assert!(args.contains(&"/srv/api".to_string()), "got: {args:?}");
}

#[test]
fn the_command_follows_a_separator() {
    let args = argv(false, &[]);
    let index = args
        .iter()
        .position(|arg| arg == "--")
        .expect("a separator is required before the command");
    assert_eq!(
        args.get(index + 1).map(String::as_str),
        Some("/usr/bin/node")
    );
    assert_eq!(args.get(index + 2).map(String::as_str), Some("server.js"));
}

#[test]
fn bubblewrap_is_looked_up_on_the_path() {
    let wrapped = bwrap_argv(BWRAP_PROGRAM, &policy(false, &[]), "/usr/bin/node", &[]);
    assert_eq!(wrapped.program, "bwrap");
}
