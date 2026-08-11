use super::*;

#[tokio::test]
async fn a_status_query_reports_a_service_that_was_never_installed() {
    let fixture = fixture(FALSE_PROGRAM);
    let home = home_of(&fixture);
    let report = dispatch_service(
        &fixture.config_path,
        None,
        &context(&fixture, UnitKind::Launchd, &home),
    )
    .await
    .expect("an absent unit needs no manager");
    assert!(report.contains("not installed"), "got: {report}");
}

#[tokio::test]
async fn a_status_query_that_cannot_reach_the_manager_is_reported() {
    let fixture = fixture(MISSING_PROGRAM);
    let home = home_of(&fixture);
    install_unit(&fixture, UnitKind::Launchd);
    let err = dispatch_service(
        &fixture.config_path,
        None,
        &context(&fixture, UnitKind::Launchd, &home),
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(err.starts_with("cannot run '/nonexistent/"), "got: {err}");
}

#[tokio::test]
async fn a_broken_config_stops_a_service_command() {
    let fixture = fixture(TRUE_PROGRAM);
    let home = home_of(&fixture);
    let err = dispatch_service(
        "/nonexistent/pm3-service.yaml",
        None,
        &context(&fixture, UnitKind::Launchd, &home),
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(
        err.starts_with("cannot resolve the config path"),
        "got: {err}"
    );
}

#[tokio::test]
async fn an_install_writes_the_unit_under_the_given_home() {
    let fixture = fixture(TRUE_PROGRAM);
    let home = home_of(&fixture);
    let command = ServiceCommands::Install {
        dry_run: false,
        force: false,
    };
    let report = dispatch_service(
        &fixture.config_path,
        Some(&command),
        &context(&fixture, UnitKind::Systemd, &home),
    )
    .await
    .expect("the install should succeed");
    assert!(report.contains("installed"), "got: {report}");
    let unit = fixture
        .dir
        .path()
        .join(".config/systemd/user")
        .join(format!("{SERVICE_LABEL}.service"));
    assert!(unit.is_file(), "{} should exist", unit.display());
}

#[tokio::test]
async fn an_install_settles_the_config_into_the_pm3_home() {
    let fixture = fixture(TRUE_PROGRAM);
    let home = home_of(&fixture);
    let settled = installed_config(&fixture, UnitKind::Systemd, &home)
        .await
        .expect("the install should succeed");
    assert_eq!(
        settled,
        std::fs::read_to_string(&fixture.config_path).expect("read the source config")
    );
}

#[tokio::test]
async fn a_schtasks_install_writes_the_task_xml_and_the_wrapper() {
    let fixture = fixture(TRUE_PROGRAM);
    let home = home_of(&fixture);
    let report = install_with(&fixture, UnitKind::WinSchtasks, &home, false)
        .await
        .expect("the install should succeed");
    assert!(report.contains("installed"), "got: {report}");
    let dir = fixture.dir.path().join(".pm3/service");
    assert!(
        dir.join(format!("{SERVICE_LABEL}.xml")).is_file(),
        "the task xml should exist"
    );
    assert!(
        dir.join(format!("{SERVICE_LABEL}-daemon.cmd")).is_file(),
        "the wrapper should exist"
    );
}

#[tokio::test]
async fn an_install_points_the_unit_at_the_settled_config() {
    let fixture = fixture(TRUE_PROGRAM);
    let home = home_of(&fixture);
    let session = open_service_session(
        &fixture.config_path,
        &context(&fixture, UnitKind::Systemd, &home),
    )
    .expect("the session should open");
    assert_eq!(session.spec.config_path, session.paths.config_file);
}

#[tokio::test]
async fn an_install_over_a_changed_config_needs_force() {
    let fixture = fixture(TRUE_PROGRAM);
    let home = home_of(&fixture);
    seed_settled_config(&fixture, UnitKind::Systemd, &home, "pm3: {}\n");
    let err = install_with(&fixture, UnitKind::Systemd, &home, false)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("without --force"), "got: {err}");
}

#[tokio::test]
async fn force_replaces_a_changed_config() {
    let fixture = fixture(TRUE_PROGRAM);
    let home = home_of(&fixture);
    seed_settled_config(&fixture, UnitKind::Systemd, &home, "pm3: {}\n");
    install_with(&fixture, UnitKind::Systemd, &home, true)
        .await
        .expect("force should overwrite the settled config");
    assert_eq!(
        settled_config(&fixture, UnitKind::Systemd, &home),
        std::fs::read_to_string(&fixture.config_path).expect("read the source config")
    );
}

#[tokio::test]
async fn a_second_install_of_the_same_config_is_accepted() {
    let fixture = fixture(TRUE_PROGRAM);
    let home = home_of(&fixture);
    installed_config(&fixture, UnitKind::Systemd, &home)
        .await
        .expect("the first install should succeed");
    install_with(&fixture, UnitKind::Systemd, &home, false)
        .await
        .expect("an unchanged config needs no force");
}

#[tokio::test]
async fn a_dry_run_install_writes_nothing() {
    let fixture = fixture(FALSE_PROGRAM);
    let home = home_of(&fixture);
    let command = ServiceCommands::Install {
        dry_run: true,
        force: false,
    };
    let report = dispatch_service(
        &fixture.config_path,
        Some(&command),
        &context(&fixture, UnitKind::Launchd, &home),
    )
    .await
    .expect("a dry run should never fail");
    assert!(report.contains("<key>RunAtLoad</key>"), "got: {report}");
    assert!(
        !fixture.dir.path().join("Library").exists(),
        "a dry run must not create the unit directory"
    );
}

#[tokio::test]
async fn an_install_that_the_manager_refuses_is_reported() {
    let fixture = fixture(FALSE_PROGRAM);
    let home = home_of(&fixture);
    let command = ServiceCommands::Install {
        dry_run: false,
        force: false,
    };
    let err = dispatch_service(
        &fixture.config_path,
        Some(&command),
        &context(&fixture, UnitKind::Launchd, &home),
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(err.contains("exited with status 1"), "got: {err}");
}

#[tokio::test]
async fn an_install_that_cannot_prepare_the_home_is_reported() {
    let dir = tempfile::tempdir().expect("temp dir");
    let home = dir.path().join("home");
    std::fs::write(&home, "blocked").expect("occupy the pm3 home");
    let config = write_config(dir.path(), &home.to_string_lossy());
    let programs = UnitProgramSet {
        launchctl: TRUE_PROGRAM.to_string(),
        systemctl: TRUE_PROGRAM.to_string(),
        loginctl: TRUE_PROGRAM.to_string(),
        schtasks: TRUE_PROGRAM.to_string(),
        runtime_dir: None,
        uid: None,
    };
    let fake_home = dir.path().to_string_lossy().into_owned();
    let context = ServiceContext {
        programs: Some(&programs),
        kind: UnitKind::Launchd,
        pm3_env: Vec::new(),
        home_env: Some(&fake_home),
        runtime_dir: None,
        uid: None,
        binary: Ok(PathBuf::from("/usr/local/bin/pm3")),
    };
    let command = ServiceCommands::Install {
        dry_run: false,
        force: false,
    };
    let err = dispatch_service(&config.to_string_lossy(), Some(&command), &context)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot prepare the pm3 home"), "got: {err}");
}

#[tokio::test]
async fn an_uninstall_removes_the_unit_it_installed() {
    let fixture = fixture(TRUE_PROGRAM);
    let home = home_of(&fixture);
    install_unit(&fixture, UnitKind::Launchd);
    let command = ServiceCommands::Uninstall { dry_run: false };
    let report = dispatch_service(
        &fixture.config_path,
        Some(&command),
        &context(&fixture, UnitKind::Launchd, &home),
    )
    .await
    .expect("the uninstall should succeed");
    assert!(report.contains("uninstalled"), "got: {report}");
}

#[path = "service_status_tests.rs"]
mod status;

#[tokio::test]
async fn an_install_renders_the_network_wait_into_the_unit() {
    let fixture = fixture(TRUE_PROGRAM);
    let home = home_of(&fixture);
    let yaml = crate::test_support::config_yaml(&fixture.dir.path().join("home").to_string_lossy())
        .replace("wait_for_network: false", "wait_for_network: true");
    std::fs::write(&fixture.config_path, yaml).expect("rewrite the config");
    let command = ServiceCommands::Install {
        dry_run: true,
        force: false,
    };
    let report = dispatch_service(
        &fixture.config_path,
        Some(&command),
        &context(&fixture, UnitKind::Systemd, &home),
    )
    .await
    .expect("a dry run should never fail");
    assert!(
        report.contains("After=network-online.target"),
        "got: {report}"
    );
}
