#[cfg(unix)]
use adapters::NOTHING_INSTALLED;
use adapters::UnitProgramSet;

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
        schtasks: program.to_string(),
        runtime_dir: None,
        uid: None,
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
        pm3_env: Vec::new(),
        home_env: Some(home),
        runtime_dir: None,
        uid: None,
        binary: Ok(PathBuf::from("/usr/local/bin/pm3")),
    }
}

#[test]
fn a_session_hands_the_host_session_to_the_service_manager() {
    let fixture = fixture(TRUE_PROGRAM);
    let home = home_of(&fixture);
    let context = ServiceContext {
        programs: None,
        kind: UnitKind::Systemd,
        pm3_env: Vec::new(),
        home_env: Some(&home),
        runtime_dir: Some("/run/user/4242".to_string()),
        uid: Some(4242),
        binary: Ok(PathBuf::from("/usr/local/bin/pm3")),
    };

    let session =
        open_service_session(&fixture.config_path, &context).expect("the session should open");

    assert_eq!(
        session.programs.runtime_dir.as_deref(),
        Some("/run/user/4242")
    );
    assert_eq!(session.programs.uid, Some(4242));
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
    let command = ServiceAction::Install {
        dry_run: false,
        force,
    };
    dispatch_service(
        &fixture.config_path,
        &command,
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
        pm3_env: Vec::new(),
        home_env: None,
        runtime_dir: None,
        uid: None,
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
        pm3_env: Vec::new(),
        home_env: Some(&home),
        runtime_dir: None,
        uid: None,
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

#[path = "service_render_tests.rs"]
mod render;
