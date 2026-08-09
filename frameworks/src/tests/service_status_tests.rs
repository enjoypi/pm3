#![cfg(unix)]
use super::*;

#[tokio::test]
async fn an_uninstall_that_cannot_remove_the_unit_is_reported() {
    let dir = tempfile::tempdir().expect("temp dir");
    let vanishing = dir.path().join("launchctl");
    std::fs::write(&vanishing, "#!/bin/sh\nrm -f \"$3\"\n").expect("write a fake launchctl");
    std::fs::set_permissions(
        &vanishing,
        <std::fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o755),
    )
    .expect("make the fake launchctl executable");
    let fixture = fixture(&vanishing.to_string_lossy());
    let home = home_of(&fixture);
    install_unit(&fixture, UnitKind::Launchd);
    let command = ServiceCommands::Uninstall { dry_run: false };

    let err = dispatch_service(
        &fixture.config_path,
        Some(&command),
        &context(&fixture, UnitKind::Launchd, &home),
    )
    .await
    .unwrap_err()
    .to_string();

    assert!(err.contains("cannot write"), "got: {err}");
}

#[tokio::test]
async fn an_uninstall_that_the_manager_refuses_reports_what_it_skipped() {
    let fixture = fixture(FALSE_PROGRAM);
    let home = home_of(&fixture);
    install_unit(&fixture, UnitKind::Launchd);
    let command = ServiceCommands::Uninstall { dry_run: false };
    let report = dispatch_service(
        &fixture.config_path,
        Some(&command),
        &context(&fixture, UnitKind::Launchd, &home),
    )
    .await
    .expect("a refusal to unload must not strand the unit file");
    assert!(report.contains("skipped: "), "got: {report}");
}

#[tokio::test]
async fn an_uninstall_without_an_install_says_so() {
    let fixture = fixture(FALSE_PROGRAM);
    let home = home_of(&fixture);
    let command = ServiceCommands::Uninstall { dry_run: false };
    let report = dispatch_service(
        &fixture.config_path,
        Some(&command),
        &context(&fixture, UnitKind::Launchd, &home),
    )
    .await
    .expect("a missing service is not an error");
    assert_eq!(report, NOTHING_INSTALLED);
}

#[tokio::test]
async fn the_host_service_query_reaches_the_real_platform_manager() {
    let dir = tempfile::tempdir().expect("temp dir");
    let home = dir.path().join("home");
    let config = write_config(dir.path(), &home.to_string_lossy());
    let report = run_service(&config.to_string_lossy(), None)
        .await
        .expect("an absent unit needs no manager");
    assert!(report.contains("not installed"), "got: {report}");
}

#[test]
fn a_relative_service_directory_stops_the_session() {
    let fixture = fixture(TRUE_PROGRAM);
    let home = home_of(&fixture);
    let config = crate::test_support::write_config_with_cfg_dir(
        fixture.dir.path(),
        "/tmp/pm3-service",
        "relative/service",
    );
    let err = open_service_session(
        &config.to_string_lossy(),
        &context(&fixture, UnitKind::Launchd, &home),
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("must be absolute"), "got: {err}");
}
