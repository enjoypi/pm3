use super::*;

#[test]
fn a_present_backend_is_selected() {
    assert!(probe_backend(&|_program| true).is_some());
}

#[test]
fn no_backend_is_selected_when_none_is_installed() {
    assert!(probe_backend(&|_program| false).is_none());
}

#[test]
fn the_host_offers_a_usable_sandbox_backend() {
    assert!(
        detect_host_backend().is_some(),
        "macOS needs /usr/bin/sandbox-exec, Linux needs bubblewrap on PATH"
    );
}
