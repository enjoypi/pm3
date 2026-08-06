use usecases::{ReadScope, SandboxMode};

use super::{super::backend::HostSandbox, *};

const BWRAP_PROGRAM: &str = "bwrap";
const SEATBELT_PROGRAM: &str = "/usr/bin/sandbox-exec";

const fn program_of(backend: SandboxBackend) -> &'static str {
    match backend {
        SandboxBackend::Seatbelt => SEATBELT_PROGRAM,
        SandboxBackend::Bwrap => BWRAP_PROGRAM,
    }
}

fn host(backend: SandboxBackend) -> HostSandbox {
    HostSandbox {
        backend,
        program: program_of(backend).to_string(),
    }
}

fn wrapper(host: Option<HostSandbox>) -> SandboxCommandWrapper {
    SandboxCommandWrapper::new(host, vec!["/usr".to_string()])
}

fn policy(mode: SandboxMode, writable_roots: &[&str]) -> SandboxPolicy {
    SandboxPolicy {
        mode,
        read: ReadScope::Full,
        network: false,
        writable_roots: writable_roots.iter().map(|r| (*r).to_string()).collect(),
        readable_roots: Vec::new(),
        derived_roots: Vec::new(),
        unreadable_roots: Vec::new(),
    }
}

#[test]
fn a_seatbelt_backend_wraps_with_sandbox_exec() {
    let sandbox = wrapper(Some(host(SandboxBackend::Seatbelt)));
    let wrapped = sandbox
        .wrap(
            "api",
            &policy(SandboxMode::WorkspaceWrite, &[]),
            "/usr/bin/node",
            &[],
        )
        .expect("seatbelt should wrap");
    assert_eq!(wrapped.program, SEATBELT_PROGRAM);
}

#[test]
fn a_bwrap_backend_wraps_with_bubblewrap() {
    let sandbox = wrapper(Some(host(SandboxBackend::Bwrap)));
    let wrapped = sandbox
        .wrap(
            "api",
            &policy(SandboxMode::WorkspaceWrite, &[]),
            "/usr/bin/node",
            &[],
        )
        .expect("bwrap should wrap");
    assert_eq!(wrapped.program, BWRAP_PROGRAM);
}

#[test]
fn a_bwrap_backend_lays_the_configured_read_allowlist_in() {
    let sandbox = wrapper(Some(host(SandboxBackend::Bwrap)));
    let confined = SandboxPolicy {
        read: ReadScope::Minimal,
        ..policy(SandboxMode::WorkspaceWrite, &[])
    };
    let wrapped = sandbox
        .wrap("api", &confined, "/usr/bin/node", &[])
        .expect("bwrap should wrap");
    assert!(
        wrapped.args.contains(&"/usr".to_string()),
        "got: {:?}",
        wrapped.args
    );
}

#[test]
fn a_missing_backend_refuses_to_start_a_confined_app() {
    let sandbox = wrapper(None);
    let err = sandbox
        .wrap(
            "api",
            &policy(SandboxMode::WorkspaceWrite, &[]),
            "/usr/bin/node",
            &[],
        )
        .unwrap_err();
    assert!(
        matches!(err, SandboxError::NoBackend { ref app } if app == "api"),
        "got: {err}"
    );
}

#[test]
fn a_missing_backend_also_refuses_a_read_only_app() {
    let sandbox = wrapper(None);
    let err = sandbox
        .wrap("api", &policy(SandboxMode::ReadOnly, &[]), "/bin/ls", &[])
        .unwrap_err();
    assert!(matches!(err, SandboxError::NoBackend { .. }), "got: {err}");
}

#[test]
fn an_unconfined_app_runs_unwrapped_even_without_a_backend() {
    let sandbox = wrapper(None);
    let wrapped = sandbox
        .wrap(
            "api",
            &policy(SandboxMode::DangerFullAccess, &[]),
            "/usr/bin/node",
            &["server.js".to_string()],
        )
        .expect("an unconfined app needs no backend");
    assert_eq!(wrapped.program, "/usr/bin/node");
    assert_eq!(wrapped.args, ["server.js".to_string()]);
}

#[test]
fn seatbelt_carries_an_awkward_writable_root_as_a_parameter_instead_of_escaping_it() {
    let sandbox = wrapper(Some(host(SandboxBackend::Seatbelt)));
    let wrapped = sandbox
        .wrap(
            "api",
            &policy(SandboxMode::WorkspaceWrite, &["/srv/a\"b\nc"]),
            "/usr/bin/node",
            &[],
        )
        .expect("a parameter needs no escaping");
    assert!(
        wrapped
            .args
            .contains(&"WRITABLE_0=/srv/a\"b\nc".to_string()),
        "got: {:?}",
        wrapped.args
    );
}

#[test]
fn bwrap_accepts_a_writable_root_that_a_seatbelt_profile_cannot_render() {
    let sandbox = wrapper(Some(host(SandboxBackend::Bwrap)));
    let wrapped = sandbox
        .wrap(
            "api",
            &policy(SandboxMode::WorkspaceWrite, &["/srv/a\nb"]),
            "/usr/bin/node",
            &[],
        )
        .expect("bwrap passes roots as argv, so a newline is harmless");
    assert!(
        wrapped.args.contains(&"/srv/a\nb".to_string()),
        "got: {:?}",
        wrapped.args
    );
}

#[test]
fn the_configured_backend_is_reported_for_logging() {
    let sandbox = wrapper(Some(host(SandboxBackend::Bwrap)));
    assert_eq!(sandbox.backend(), Some(SandboxBackend::Bwrap));
}
