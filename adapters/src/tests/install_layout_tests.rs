use super::*;

#[test]
fn a_declared_destination_wins() {
    let destination = destination_of(Some("/opt/pm3"), Some("/home/dev")).expect("a declared path");
    assert_eq!(destination, std::path::Path::new("/opt/pm3"));
}

#[test]
fn an_empty_destination_falls_back_to_the_home_bin() {
    let destination = destination_of(Some(""), Some("/home/dev")).expect("home is known");
    assert_eq!(destination, std::path::Path::new("/home/dev/bin/pm3"));
}

#[test]
fn an_undeclared_destination_falls_back_to_the_home_bin() {
    let destination = destination_of(None, Some("/home/dev")).expect("home is known");
    assert_eq!(destination, std::path::Path::new("/home/dev/bin/pm3"));
}

#[test]
fn a_destination_without_a_home_is_an_error() {
    let error = destination_of(None, None).unwrap_err();
    assert_eq!(
        error.to_string(),
        "cannot resolve the install destination: no HOME in the environment"
    );
}

#[test]
fn a_declared_backup_root_wins() {
    let root = backup_root(Some("/srv/backups"), std::path::Path::new("/home/dev/.pm3"));
    assert_eq!(root, std::path::Path::new("/srv/backups"));
}

#[test]
fn an_empty_backup_root_lives_under_the_pm3_home() {
    let root = backup_root(Some(""), std::path::Path::new("/home/dev/.pm3"));
    assert_eq!(root, std::path::Path::new("/home/dev/.pm3/install-backups"));
}

#[test]
fn an_undeclared_backup_root_lives_under_the_pm3_home() {
    let root = backup_root(None, std::path::Path::new("/home/dev/.pm3"));
    assert_eq!(root, std::path::Path::new("/home/dev/.pm3/install-backups"));
}

#[test]
fn a_backup_name_is_the_old_version() {
    assert_eq!(backup_name(Some("1.8.0")), "1.8.0");
}

#[test]
fn a_backup_name_falls_back_to_unknown() {
    assert_eq!(backup_name(None), "unknown");
    assert_eq!(backup_name(Some("")), "unknown");
}

#[test]
fn a_backup_name_rejects_path_unsafe_versions() {
    assert_eq!(backup_name(Some("../etc")), "unknown");
    assert_eq!(backup_name(Some("a/b")), "unknown");
}

#[test]
fn a_version_output_yields_its_last_token() {
    assert_eq!(parse_version_output("pm3 1.8.0\n"), Some("1.8.0"));
}

#[test]
fn a_versionless_output_yields_nothing() {
    assert_eq!(parse_version_output(""), None);
    assert_eq!(parse_version_output("pm3 dirty/tree\n"), None);
}
