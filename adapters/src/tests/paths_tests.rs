use super::*;

#[test]
fn the_default_config_lives_in_the_config_root_without_a_pm3_home() {
    let path = default_config_path(None, None, None, Some("/home/u"))
        .expect("the default config path resolves");
    assert_eq!(path, Path::new("/home/u/.config/pm3/config.yaml"));
}

#[test]
fn a_pm3_home_in_the_environment_keeps_the_single_root_layout() {
    let path = default_config_path(Some("/srv/pm3"), None, None, Some("/home/u"))
        .expect("the default config path resolves");
    assert_eq!(path, Path::new("/srv/pm3/config.yaml"));
}

#[test]
fn an_empty_pm3_home_falls_back_to_the_config_root() {
    let path = default_config_path(Some(""), None, None, Some("/home/u"))
        .expect("the default config path resolves");
    assert_eq!(path, Path::new("/home/u/.config/pm3/config.yaml"));
}

#[test]
fn a_config_directory_variable_moves_the_default_config() {
    let path = default_config_path(None, Some("/srv/cfg"), None, Some("/home/u"))
        .expect("the default config path resolves");
    assert_eq!(path, Path::new("/srv/cfg/config.yaml"));
}

#[test]
fn the_default_config_needs_a_home_environment() {
    let err = default_config_path(None, None, None, None)
        .unwrap_err()
        .to_string();
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

#[test]
fn the_single_root_layout_keeps_every_path_where_it_was() {
    let paths = resolve_paths(Pm3Roots::single(Path::new("/home/u/.pm3")));
    assert_eq!(paths.root, Path::new("/home/u/.pm3"));
    assert_eq!(paths.socket, Path::new("/home/u/.pm3/pm3.sock"));
    assert_eq!(paths.pid_file, Path::new("/home/u/.pm3/pm3.pid"));
    assert_eq!(paths.lock_file, Path::new("/home/u/.pm3/pm3.lock"));
    assert_eq!(paths.config_file, Path::new("/home/u/.pm3/config.yaml"));
    assert_eq!(paths.dump_file, Path::new("/home/u/.pm3/dump.yaml"));
    assert_eq!(paths.logs_dir, Path::new("/home/u/.pm3/logs"));
    assert_eq!(paths.daemon_log, Path::new("/home/u/.pm3/pm3.log"));
    assert_eq!(paths.apps_dir, Path::new("/home/u/.pm3"));
    assert_eq!(paths.backups_dir, Path::new("/home/u/.pm3/install-backups"));
}

#[test]
fn the_split_layout_puts_every_file_under_the_root_that_owns_it() {
    let paths = resolve_paths(Pm3Roots::split(
        PathBuf::from("/c/pm3"),
        PathBuf::from("/s/pm3"),
        PathBuf::from("/r/pm3"),
        PathBuf::from("/d/pm3"),
    ));
    assert_eq!(paths.config_file, Path::new("/c/pm3/config.yaml"));
    assert_eq!(paths.dump_file, Path::new("/s/pm3/dump.yaml"));
    assert_eq!(paths.logs_dir, Path::new("/s/pm3/logs"));
    assert_eq!(paths.daemon_log, Path::new("/s/pm3/pm3.log"));
    assert_eq!(paths.apps_dir, Path::new("/s/pm3/apps"));
    assert_eq!(paths.socket, Path::new("/r/pm3/pm3.sock"));
    assert_eq!(paths.pid_file, Path::new("/r/pm3/pm3.pid"));
    assert_eq!(paths.lock_file, Path::new("/r/pm3/pm3.lock"));
    assert_eq!(paths.backups_dir, Path::new("/d/pm3/install-backups"));
    assert_eq!(paths.root, Path::new("/s/pm3"), "the daemon works in state");
}

#[test]
fn the_config_root_prefers_the_explicit_variable_over_the_xdg_one() {
    let root = resolve_config_root(Some("/srv/cfg"), Some("/x/config"), Some("/home/u"))
        .expect("an absolute override resolves");
    assert_eq!(root, Path::new("/srv/cfg"), "no pm3 suffix on an override");
}

#[test]
fn the_config_root_takes_the_xdg_variable_when_pm3_names_none() {
    let root = resolve_config_root(None, Some("/x/config"), Some("/home/u"))
        .expect("the xdg variable resolves");
    assert_eq!(root, Path::new("/x/config/pm3"));
}

#[test]
fn the_config_root_falls_back_to_the_dot_config_directory() {
    let root = resolve_config_root(None, None, Some("/home/u")).expect("the fallback resolves");
    assert_eq!(root, Path::new("/home/u/.config/pm3"));
}

#[test]
fn the_state_root_falls_back_to_the_local_state_directory() {
    let root = resolve_state_root(None, None, Some("/home/u")).expect("the fallback resolves");
    assert_eq!(root, Path::new("/home/u/.local/state/pm3"));
}

#[test]
fn the_data_root_falls_back_to_the_local_share_directory() {
    let root = resolve_data_root(None, None, Some("/home/u")).expect("the fallback resolves");
    assert_eq!(root, Path::new("/home/u/.local/share/pm3"));
}

#[test]
fn an_empty_variable_counts_as_unset() {
    let root =
        resolve_state_root(Some(""), Some(""), Some("/home/u")).expect("the fallback resolves");
    assert_eq!(root, Path::new("/home/u/.local/state/pm3"));
}

#[test]
fn the_runtime_root_prefers_the_explicit_variable() {
    let root = resolve_runtime_root(&RuntimeSources {
        declared: Some("/srv/run"),
        xdg: Some("/x/run"),
        uid: Some(1000),
        state: Path::new("/s/pm3"),
        exists: |_| true,
    });
    assert_eq!(root, Path::new("/srv/run"));
}

#[test]
fn the_runtime_root_takes_the_xdg_runtime_directory_when_it_is_declared() {
    let root = resolve_runtime_root(&RuntimeSources {
        declared: None,
        xdg: Some("/x/run"),
        uid: Some(1000),
        state: Path::new("/s/pm3"),
        exists: |_| false,
    });
    assert_eq!(root, Path::new("/x/run/pm3"));
}

#[test]
fn the_runtime_root_uses_run_user_when_the_kernel_provides_it() {
    let root = resolve_runtime_root(&RuntimeSources {
        declared: None,
        xdg: None,
        uid: Some(1000),
        state: Path::new("/s/pm3"),
        exists: |_| true,
    });
    assert_eq!(root, Path::new("/run/user/1000/pm3"));
}

#[test]
fn the_runtime_root_falls_back_to_the_state_directory_when_run_user_does_not_exist() {
    let root = resolve_runtime_root(&RuntimeSources {
        declared: None,
        xdg: None,
        uid: Some(1000),
        state: Path::new("/s/pm3"),
        exists: |_| false,
    });
    assert_eq!(root, Path::new("/s/pm3/run"));
}

#[test]
fn the_runtime_root_falls_back_to_the_state_directory_without_a_uid() {
    let root = resolve_runtime_root(&RuntimeSources {
        declared: None,
        xdg: None,
        uid: None,
        state: Path::new("/s/pm3"),
        exists: |_| true,
    });
    assert_eq!(root, Path::new("/s/pm3/run"));
}

#[test]
fn a_socket_path_within_the_limit_is_accepted() {
    check_socket_length(Path::new("/run/user/1000/pm3/pm3.sock")).expect("a short path is fine");
}

#[test]
fn a_socket_path_longer_than_the_unix_limit_is_refused() {
    let deep = format!("/{}/pm3.sock", "d".repeat(120));
    let err = check_socket_length(Path::new(&deep)).unwrap_err();
    assert!(matches!(err, PathError::SocketTooLong { .. }), "got: {err}");
    assert!(
        err.to_string().starts_with("cannot accept the socket path"),
        "got: {err}"
    );
}

#[test]
fn a_relative_config_root_override_is_refused() {
    let err = resolve_config_root(Some("cfg"), None, Some("/home/u")).unwrap_err();
    assert_eq!(err, PathError::NotAbsolute("cfg".to_string()));
}

#[test]
fn a_tilde_pm3_home_without_a_home_environment_is_refused() {
    let err = default_config_path(Some("~/.pm3"), None, None, None).unwrap_err();
    assert_eq!(err, PathError::MissingHome("~/.pm3".to_string()));
}

#[test]
fn roots_that_only_partly_agree_are_not_a_single_root() {
    let shared = PathBuf::from("/x/pm3");
    let partly = Pm3Roots::split(
        shared.clone(),
        shared.clone(),
        PathBuf::from("/r/pm3"),
        shared,
    );
    assert!(
        !partly.is_single(),
        "a runtime root of its own already means the split layout"
    );
    let paths = resolve_paths(partly);
    assert_eq!(
        paths.apps_dir,
        Path::new("/x/pm3/apps"),
        "the split layout keeps the service directories apart from the state files"
    );
}
