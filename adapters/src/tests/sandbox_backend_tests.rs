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
