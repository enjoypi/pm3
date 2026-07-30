use usecases::SandboxMode;

use super::{
    super::backend::{BWRAP_PROGRAM, SEATBELT_PROGRAM},
    *,
};

fn policy(mode: SandboxMode, writable_roots: &[&str]) -> SandboxPolicy {
    SandboxPolicy {
        mode,
        network: false,
        writable_roots: writable_roots.iter().map(|r| (*r).to_string()).collect(),
    }
}

#[test]
fn a_seatbelt_backend_wraps_with_sandbox_exec() {
    let sandbox = SandboxCommandWrapper::new(Some(SandboxBackend::Seatbelt));
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
    let sandbox = SandboxCommandWrapper::new(Some(SandboxBackend::Bwrap));
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
fn a_writable_root_with_a_quote_is_rejected() {
    let sandbox = SandboxCommandWrapper::new(Some(SandboxBackend::Seatbelt));
    let err = sandbox
        .wrap(
            "api",
            &policy(SandboxMode::WorkspaceWrite, &["/srv/\"evil"]),
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
fn a_writable_root_with_a_newline_is_rejected() {
    let sandbox = SandboxCommandWrapper::new(Some(SandboxBackend::Bwrap));
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
fn the_configured_backend_is_reported_for_logging() {
    let sandbox = SandboxCommandWrapper::new(Some(SandboxBackend::Bwrap));
    assert_eq!(sandbox.backend(), Some(SandboxBackend::Bwrap));
}
