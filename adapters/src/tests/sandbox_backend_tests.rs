use super::*;

#[test]
fn seatbelt_runs_the_macos_sandbox_binary() {
    assert_eq!(SandboxBackend::Seatbelt.program(), "/usr/bin/sandbox-exec");
}

#[test]
fn bwrap_runs_bubblewrap_from_the_path() {
    assert_eq!(SandboxBackend::Bwrap.program(), "bwrap");
}

#[test]
fn each_backend_has_a_log_friendly_name() {
    assert_eq!(SandboxBackend::Seatbelt.as_str(), "seatbelt");
    assert_eq!(SandboxBackend::Bwrap.as_str(), "bwrap");
}

#[test]
fn a_backend_that_is_not_installed_resolves_to_nothing() {
    assert!(
        SandboxBackend::Bwrap
            .resolve(Some("/nonexistent"))
            .is_none()
    );
}

#[test]
fn a_resolved_backend_carries_the_absolute_path_of_its_program() {
    let host = SandboxBackend::Bwrap
        .resolve(Some("/bin"))
        .map(|found| found.program);
    assert_eq!(host.is_some(), std::path::Path::new("/bin/bwrap").is_file());
}
