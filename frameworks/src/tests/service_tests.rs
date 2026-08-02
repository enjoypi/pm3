use adapters::{NOTHING_INSTALLED, UnitProgramSet};

use super::*;
use crate::test_support::{
    SERVICE_LABEL, SERVICE_RESTART_DELAY_SECS, SERVICE_SEARCH_PATH, write_config,
};

const TRUE_PROGRAM: &str = "/usr/bin/true";
const FALSE_PROGRAM: &str = "/usr/bin/false";
const MISSING_PROGRAM: &str = "/nonexistent/pm3-service-manager";

struct Fixture {
    dir: tempfile::TempDir,
    config_path: String,
    programs: UnitProgramSet,
}

fn fixture(program: &str) -> Fixture {
    let dir = tempfile::tempdir().expect("temp dir");
    let home = dir.path().join("home");
    let config = write_config(dir.path(), &home.to_string_lossy());
    let config_path = config.to_string_lossy().into_owned();
    let programs = UnitProgramSet {
        launchctl: program.to_string(),
        systemctl: program.to_string(),
        loginctl: program.to_string(),
    };
    Fixture {
        dir,
        config_path,
        programs,
    }
}

fn context<'c>(fixture: &'c Fixture, kind: UnitKind, home: &'c str) -> ServiceContext<'c> {
    ServiceContext {
        programs: Some(&fixture.programs),
        kind,
        home_env: Some(home),
        binary: Ok(PathBuf::from("/usr/local/bin/pm3")),
    }
}

fn home_of(fixture: &Fixture) -> String {
    fixture.dir.path().to_string_lossy().into_owned()
}

fn settled_path(fixture: &Fixture, kind: UnitKind, home: &str) -> PathBuf {
    open_service_session(&fixture.config_path, &context(fixture, kind, home))
        .expect("the session should open")
        .spec
        .config_path
}

fn seed_settled_config(fixture: &Fixture, kind: UnitKind, home: &str, body: &str) {
    let path = settled_path(fixture, kind, home);
    std::fs::create_dir_all(path.parent().expect("the pm3 home")).expect("prepare the pm3 home");
    std::fs::write(path, body).expect("seed a settled config");
}

fn settled_config(fixture: &Fixture, kind: UnitKind, home: &str) -> String {
    std::fs::read_to_string(settled_path(fixture, kind, home)).expect("read the settled config")
}

async fn install_with(
    fixture: &Fixture,
    kind: UnitKind,
    home: &str,
    force: bool,
) -> Result<String> {
    let command = ServiceCommands::Install {
        dry_run: false,
        force,
    };
    dispatch_service(
        &fixture.config_path,
        Some(&command),
        &context(fixture, kind, home),
    )
    .await
}

async fn installed_config(fixture: &Fixture, kind: UnitKind, home: &str) -> Result<String> {
    install_with(fixture, kind, home, false).await?;
    Ok(settled_config(fixture, kind, home))
}

fn install_unit(fixture: &Fixture, kind: UnitKind) {
    let home = home_of(fixture);
    let session = open_service_session(&fixture.config_path, &context(fixture, kind, &home))
        .expect("the session should open");
    std::fs::create_dir_all(&session.spec.unit_dir).expect("prepare the unit directory");
    std::fs::write(session.spec.unit_path(), "unit body").expect("install a unit file");
}

#[test]
fn the_spec_carries_absolute_paths_and_the_daemon_invocation() {
    let fixture = fixture(TRUE_PROGRAM);
    let home = home_of(&fixture);
    let session = open_service_session(
        &fixture.config_path,
        &context(&fixture, UnitKind::Launchd, &home),
    )
    .expect("the session should open");
    assert!(session.spec.program.is_absolute(), "the binary path");
    assert!(session.spec.config_path.is_absolute(), "the config path");
    assert!(session.spec.unit_path().starts_with(&home), "the unit path");
    assert_eq!(
        session.spec.daemon_args()[0..2],
        ["daemon".to_string(), "--config".to_string()]
    );
}

#[test]
fn the_spec_takes_the_label_and_search_path_from_the_config() {
    let fixture = fixture(TRUE_PROGRAM);
    let home = home_of(&fixture);
    let session = open_service_session(
        &fixture.config_path,
        &context(&fixture, UnitKind::Systemd, &home),
    )
    .expect("the session should open");
    assert_eq!(session.spec.label, SERVICE_LABEL);
    assert_eq!(session.spec.search_path, SERVICE_SEARCH_PATH);
}

#[test]
fn the_spec_takes_the_restart_delay_from_the_config() {
    let fixture = fixture(TRUE_PROGRAM);
    let home = home_of(&fixture);
    let session = open_service_session(
        &fixture.config_path,
        &context(&fixture, UnitKind::Systemd, &home),
    )
    .expect("the session should open");
    assert_eq!(session.spec.restart_delay_secs, SERVICE_RESTART_DELAY_SECS);
}

#[test]
fn a_missing_home_stops_the_session() {
    let fixture = fixture(TRUE_PROGRAM);
    let context = ServiceContext {
        programs: Some(&fixture.programs),
        kind: UnitKind::Launchd,
        home_env: None,
        binary: Ok(PathBuf::from("/usr/local/bin/pm3")),
    };
    let err = open_service_session(&fixture.config_path, &context)
        .unwrap_err()
        .to_string();
    assert!(err.contains("no HOME in the environment"), "got: {err}");
}

#[test]
fn a_binary_that_cannot_be_located_stops_the_session() {
    let fixture = fixture(TRUE_PROGRAM);
    let home = home_of(&fixture);
    let context = ServiceContext {
        programs: Some(&fixture.programs),
        kind: UnitKind::Launchd,
        home_env: Some(&home),
        binary: Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "no such process image",
        )),
    };
    let err = open_service_session(&fixture.config_path, &context)
        .unwrap_err()
        .to_string();
    assert!(
        err.starts_with("cannot determine the pm3 binary path"),
        "got: {err}"
    );
}

#[test]
fn a_config_path_that_leads_nowhere_stops_the_session() {
    let fixture = fixture(TRUE_PROGRAM);
    let home = home_of(&fixture);
    let err = open_service_session(
        "/nonexistent/pm3-service.yaml",
        &context(&fixture, UnitKind::Launchd, &home),
    )
    .unwrap_err()
    .to_string();
    assert!(
        err.starts_with("cannot resolve the config path"),
        "got: {err}"
    );
}

#[test]
fn an_unreadable_config_stops_the_session() {
    let fixture = fixture(TRUE_PROGRAM);
    let home = home_of(&fixture);
    let broken = fixture.dir.path().join("broken.yaml");
    std::fs::write(&broken, "pm3: [not, a, mapping]\n").expect("write a broken config");
    let err = open_service_session(
        &broken.to_string_lossy(),
        &context(&fixture, UnitKind::Launchd, &home),
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("cannot parse config"), "got: {err}");
}

#[test]
fn a_relative_pm3_home_stops_the_session() {
    let fixture = fixture(TRUE_PROGRAM);
    let home = home_of(&fixture);
    let config = write_config(fixture.dir.path(), "relative/home");
    let err = open_service_session(
        &config.to_string_lossy(),
        &context(&fixture, UnitKind::Launchd, &home),
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("must be absolute"), "got: {err}");
}

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
    };
    let fake_home = dir.path().to_string_lossy().into_owned();
    let context = ServiceContext {
        programs: Some(&programs),
        kind: UnitKind::Launchd,
        home_env: Some(&fake_home),
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
