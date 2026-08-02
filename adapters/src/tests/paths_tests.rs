use super::*;

#[test]
fn every_pm3_file_lives_under_the_root() {
    let paths = resolve_paths(Path::new("/home/u/.pm3"));
    assert_eq!(paths.root, Path::new("/home/u/.pm3"));
    assert_eq!(paths.socket, Path::new("/home/u/.pm3/pm3.sock"));
    assert_eq!(paths.pid_file, Path::new("/home/u/.pm3/pm3.pid"));
    assert_eq!(paths.lock_file, Path::new("/home/u/.pm3/pm3.lock"));
    assert_eq!(paths.config_file, Path::new("/home/u/.pm3/config.yaml"));
    assert_eq!(paths.dump_file, Path::new("/home/u/.pm3/dump.yaml"));
    assert_eq!(paths.logs_dir, Path::new("/home/u/.pm3/logs"));
    assert_eq!(paths.daemon_log, Path::new("/home/u/.pm3/pm3.log"));
}

#[test]
fn the_default_config_lives_in_the_default_home() {
    let path = default_config_path(Some("/home/u")).expect("the default config path resolves");
    assert_eq!(path, Path::new("/home/u/.pm3/config.yaml"));
}

#[test]
fn the_default_config_needs_a_home_environment() {
    let err = default_config_path(None).unwrap_err().to_string();
    assert!(err.contains("no HOME in the environment"), "got: {err}");
}

#[test]
fn an_absolute_home_is_kept_as_is() {
    let resolved = expand_home("/srv/pm3", None).expect("absolute paths need no environment");
    assert_eq!(resolved, Path::new("/srv/pm3"));
}

#[test]
fn a_tilde_expands_against_the_home_environment() {
    let resolved = expand_home("~/.pm3", Some("/home/u")).expect("tilde expands");
    assert_eq!(resolved, Path::new("/home/u/.pm3"));
}

#[test]
fn a_bare_tilde_expands_to_the_home_itself() {
    let resolved = expand_home("~", Some("/home/u")).expect("tilde expands");
    assert_eq!(resolved, Path::new("/home/u"));
}

#[test]
fn a_tilde_with_a_trailing_slash_expands_to_the_home_itself() {
    let resolved = expand_home("~/", Some("/home/u")).expect("tilde expands");
    assert_eq!(resolved, Path::new("/home/u"));
}

#[test]
fn a_tilde_without_a_home_environment_is_rejected() {
    let err = expand_home("~/.pm3", None).unwrap_err();
    assert_eq!(err, PathError::MissingHome("~/.pm3".to_string()));
}

#[test]
fn a_tilde_with_an_empty_home_environment_is_rejected() {
    let err = expand_home("~/.pm3", Some("")).unwrap_err();
    assert_eq!(err, PathError::MissingHome("~/.pm3".to_string()));
}

#[test]
fn a_relative_home_is_rejected() {
    let err = expand_home("pm3-data", Some("/home/u")).unwrap_err();
    assert_eq!(err, PathError::NotAbsolute("pm3-data".to_string()));
}

#[test]
fn another_users_home_is_rejected_instead_of_expanding_against_ours() {
    let err = expand_home("~deploy/.pm3", Some("/home/u")).unwrap_err();
    assert_eq!(err, PathError::NamedHome("~deploy/.pm3".to_string()));
}

#[test]
fn every_path_error_renders_a_message() {
    let errors = [
        PathError::MissingHome("~/.pm3".to_string()),
        PathError::NotAbsolute("rel".to_string()),
        PathError::NamedHome("~deploy/.pm3".to_string()),
    ];
    for err in errors {
        assert!(
            err.to_string().starts_with("cannot resolve pm3.home"),
            "got: {err}"
        );
    }
}
