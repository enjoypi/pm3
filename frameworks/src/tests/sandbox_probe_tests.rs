use super::*;

const PROBE_TARGET: &str = "pm3-probe-target";

fn directory_holding_the_target() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join(PROBE_TARGET), "#!/bin/sh\n").expect("write the target");
    dir
}

#[test]
fn an_absolute_program_is_found_on_disk() {
    assert!(program_available("/bin/sh", None));
}

#[test]
fn a_missing_absolute_program_is_not_found() {
    assert!(!program_available("/nonexistent/pm3-probe", None));
}

#[test]
fn a_bare_program_is_found_through_the_search_path() {
    let dir = directory_holding_the_target();
    let path_env = dir.path().to_string_lossy().into_owned();
    assert!(program_available(PROBE_TARGET, Some(&path_env)));
}

#[test]
fn a_bare_program_missing_from_the_search_path_is_not_found() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path_env = dir.path().to_string_lossy().into_owned();
    assert!(!program_available(PROBE_TARGET, Some(&path_env)));
}

#[test]
fn a_bare_program_without_a_search_path_is_not_found() {
    assert!(!program_available(PROBE_TARGET, None));
}

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
