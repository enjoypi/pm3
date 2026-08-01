use usecases::SandboxMode;

use super::{
    super::backend::{BWRAP_PROGRAM, HostSandbox, SEATBELT_PROGRAM},
    *,
};

fn host(backend: SandboxBackend) -> HostSandbox {
    HostSandbox {
        backend,
        program: backend.program().to_string(),
    }
}

fn policy(mode: SandboxMode, writable_roots: &[&str]) -> SandboxPolicy {
    SandboxPolicy {
        mode,
        network: false,
        writable_roots: writable_roots.iter().map(|r| (*r).to_string()).collect(),
        derived_roots: Vec::new(),
    }
}

#[test]
fn a_seatbelt_backend_wraps_with_sandbox_exec() {
    let sandbox = SandboxCommandWrapper::new(Some(host(SandboxBackend::Seatbelt)));
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
    let sandbox = SandboxCommandWrapper::new(Some(host(SandboxBackend::Bwrap)));
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
fn a_missing_backend_refuses_to_start_a_confined_app() {
    let sandbox = SandboxCommandWrapper::new(None);
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
    let sandbox = SandboxCommandWrapper::new(None);
    let err = sandbox
        .wrap("api", &policy(SandboxMode::ReadOnly, &[]), "/bin/ls", &[])
        .unwrap_err();
    assert!(matches!(err, SandboxError::NoBackend { .. }), "got: {err}");
}

#[test]
fn an_unconfined_app_runs_unwrapped_even_without_a_backend() {
    let sandbox = SandboxCommandWrapper::new(None);
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
fn seatbelt_escapes_a_writable_root_that_holds_a_quote() {
    let sandbox = SandboxCommandWrapper::new(Some(host(SandboxBackend::Seatbelt)));
    let wrapped = sandbox
        .wrap(
            "api",
            &policy(SandboxMode::WorkspaceWrite, &["/srv/\"evil"]),
            "/usr/bin/node",
            &[],
        )
        .expect("a quote should be escaped, not refused");
    let profile = wrapped.args.get(1).expect("the profile is the second arg");
    assert!(
        profile.contains(r#"(subpath "/srv/\"evil")"#),
        "got: {profile}"
    );
}

#[test]
fn seatbelt_refuses_a_writable_root_that_holds_a_newline() {
    let sandbox = SandboxCommandWrapper::new(Some(host(SandboxBackend::Seatbelt)));
    let err = sandbox
        .wrap(
            "api",
            &policy(SandboxMode::WorkspaceWrite, &["/srv/a\nb"]),
            "/usr/bin/node",
            &[],
        )
        .unwrap_err();
    assert!(
        matches!(err, SandboxError::Unsupported { .. }),
        "got: {err}"
    );
}

#[test]
fn bwrap_accepts_a_writable_root_that_a_seatbelt_profile_cannot_render() {
    let sandbox = SandboxCommandWrapper::new(Some(host(SandboxBackend::Bwrap)));
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
    let sandbox = SandboxCommandWrapper::new(Some(host(SandboxBackend::Bwrap)));
    assert_eq!(sandbox.backend(), Some(SandboxBackend::Bwrap));
}
