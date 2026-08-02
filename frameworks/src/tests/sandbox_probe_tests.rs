use super::*;

fn programs() -> SandboxProgramSet {
    SandboxProgramSet {
        seatbelt: "/usr/bin/sandbox-exec".to_string(),
        bwrap: "bwrap".to_string(),
    }
}

fn found(backend: SandboxBackend) -> HostSandbox {
    HostSandbox {
        backend,
        program: programs().program(backend).to_string(),
    }
}

#[test]
fn a_present_backend_is_selected() {
    assert!(probe_backend(&|backend| Some(found(backend))).is_some());
}

#[test]
fn no_backend_is_selected_when_none_is_installed() {
    assert!(probe_backend(&|_backend| None).is_none());
}

#[test]
fn a_resolved_backend_carries_the_absolute_program_path() {
    let host = probe_backend(&|backend| {
        Some(HostSandbox {
            backend,
            program: "/opt/pm3/bin/sandbox".to_string(),
        })
    })
    .expect("a backend should be selected");
    assert_eq!(host.program, "/opt/pm3/bin/sandbox");
}

#[test]
fn the_host_offers_a_usable_sandbox_backend() {
    assert!(
        detect_host_backend(
            &programs(),
            "/usr/bin:/bin:/usr/local/bin:/opt/homebrew/bin"
        )
        .is_some(),
        "macOS needs /usr/bin/sandbox-exec, Linux needs bubblewrap on pm3.search_path"
    );
}
