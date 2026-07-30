use super::*;

const PROBE_TARGET: &str = "pm3-probe-target";

fn directory_holding_the_target() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join(PROBE_TARGET), "#!/bin/sh\n").expect("write the target");
    dir
}

#[test]
fn an_absolute_program_resolves_to_itself() {
    assert_eq!(
        resolve_program("/bin/sh", None),
        Some(PathBuf::from("/bin/sh"))
    );
}

#[test]
fn a_missing_absolute_program_is_not_found() {
    assert!(!program_available("/nonexistent/pm3-probe", None));
}

#[test]
fn a_bare_program_resolves_through_the_search_path() {
    let dir = directory_holding_the_target();
    let path_env = dir.path().to_string_lossy().into_owned();
    assert_eq!(
        resolve_program(PROBE_TARGET, Some(&path_env)),
        Some(dir.path().join(PROBE_TARGET))
    );
}

#[test]
fn the_first_matching_search_path_entry_wins() {
    let first = directory_holding_the_target();
    let second = directory_holding_the_target();
    let path_env = format!(
        "{}:{}",
        first.path().to_string_lossy(),
        second.path().to_string_lossy()
    );
    assert_eq!(
        resolve_program(PROBE_TARGET, Some(&path_env)),
        Some(first.path().join(PROBE_TARGET))
    );
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
fn a_path_under_the_home_folds_into_a_placeholder() {
    assert_eq!(
        fold_home("/home/dev/.config/mihomo/rule.yaml", Some("/home/dev")),
        "${HOME}/.config/mihomo/rule.yaml"
    );
}

#[test]
fn the_home_itself_folds_into_a_bare_placeholder() {
    assert_eq!(fold_home("/home/dev", Some("/home/dev")), "${HOME}");
}

#[test]
fn a_path_outside_the_home_is_left_alone() {
    assert_eq!(
        fold_home("/etc/pm3.yaml", Some("/home/dev")),
        "/etc/pm3.yaml"
    );
}

#[test]
fn a_sibling_that_merely_shares_the_prefix_is_left_alone() {
    assert_eq!(
        fold_home("/home/developer/x", Some("/home/dev")),
        "/home/developer/x"
    );
}

#[test]
fn a_bare_service_cwd_token_folds_into_the_braced_form() {
    assert_eq!(fold_svc_cwd("PM3_SVC_CWD"), SVC_CWD_PLACEHOLDER);
}

#[test]
fn a_bare_service_cwd_token_folds_inside_a_larger_argument() {
    assert_eq!(fold_svc_cwd("PM3_SVC_CWD/data"), "${PM3_SVC_CWD}/data");
}

#[test]
fn an_already_braced_service_cwd_token_is_left_alone() {
    assert_eq!(fold_svc_cwd(SVC_CWD_PLACEHOLDER), SVC_CWD_PLACEHOLDER);
}

#[test]
fn a_value_without_the_service_cwd_token_is_left_alone() {
    assert_eq!(fold_svc_cwd("/srv/data"), "/srv/data");
}

#[test]
fn folding_without_a_home_leaves_the_value_alone() {
    assert_eq!(fold_home("/home/dev/x", None), "/home/dev/x");
}

#[test]
fn folding_against_an_empty_home_leaves_the_value_alone() {
    assert_eq!(fold_home("/home/dev/x", Some("")), "/home/dev/x");
}
