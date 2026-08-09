#![cfg(unix)]
use std::{
    io,
    os::unix::fs::PermissionsExt as _,
    path::{Path, PathBuf},
};

use adapters::{UnitKind, UnitProgramSet};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

use super::*;
use crate::test_support::{write_config, write_impatient_config};

const MANAGER_PID: u32 = 4242;

struct Fixture {
    dir: tempfile::TempDir,
    home: PathBuf,
    config_path: String,
    destination: PathBuf,
    backups: PathBuf,
    programs: UnitProgramSet,
}

fn fixture(dir: tempfile::TempDir, config_path: String, manager: &str) -> Fixture {
    let home = dir.path().join("home");
    std::fs::create_dir_all(&home).expect("prepare the pm3 home");
    let programs = UnitProgramSet {
        launchctl: manager.to_string(),
        systemctl: manager.to_string(),
        loginctl: manager.to_string(),
        schtasks: manager.to_string(),
        runtime_dir: None,
        uid: None,
    };
    Fixture {
        destination: dir.path().join("dest/pm3"),
        backups: dir.path().join("backups"),
        dir,
        home,
        config_path,
        programs,
    }
}

fn systemd_fixture(manager_body: &str) -> Fixture {
    let dir = tempfile::tempdir().expect("temp dir");
    let home = dir.path().join("home");
    let config = write_config(dir.path(), &home.to_string_lossy());
    let manager = fake_manager(dir.path(), manager_body);
    fixture(dir, config.to_string_lossy().into_owned(), &manager)
}

fn impatient_systemd_fixture(manager_body: &str) -> Fixture {
    let dir = tempfile::tempdir().expect("temp dir");
    let home = dir.path().join("home");
    let config = write_impatient_config(dir.path(), &home.to_string_lossy());
    let manager = fake_manager(dir.path(), manager_body);
    fixture(dir, config.to_string_lossy().into_owned(), &manager)
}

fn fake_manager(dir: &Path, body: &str) -> String {
    let path = dir.join("fake-manager");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write the fake manager");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    path.to_string_lossy().into_owned()
}

fn context(fixture: &Fixture, kind: UnitKind, uid: Option<u32>) -> InstallContext {
    context_with_exe(fixture, kind, uid, Ok(fixture.dir.path().join("new-pm3")))
}

fn context_with_exe(
    fixture: &Fixture,
    kind: UnitKind,
    uid: Option<u32>,
    exe: io::Result<PathBuf>,
) -> InstallContext {
    let mut programs = fixture.programs.clone();
    programs.uid = uid;
    InstallContext {
        home_env: Some(fixture.home.to_string_lossy().into_owned()),
        destination_env: Some(fixture.destination.to_string_lossy().into_owned()),
        backups_env: Some(fixture.backups.to_string_lossy().into_owned()),
        pm3_env: Vec::new(),
        runtime_dir: None,
        uid,
        current_exe: exe,
        kind,
        programs: Some(programs),
    }
}

fn seed_source(fixture: &Fixture) -> PathBuf {
    let source = fixture.dir.path().join("new-pm3");
    std::fs::write(&source, "new binary").expect("write the new binary");
    source
}

fn seed_pid_file(fixture: &Fixture) {
    std::fs::write(fixture.home.join("pm3.pid"), format!("{MANAGER_PID}"))
        .expect("write the pid file");
}

fn unit_file(fixture: &Fixture, kind: UnitKind) -> PathBuf {
    let dir = match kind {
        UnitKind::Launchd => fixture.home.join("Library/LaunchAgents"),
        UnitKind::Systemd => fixture.home.join(".config/systemd/user"),
        UnitKind::WinSchtasks => fixture.home.join(".pm3/service"),
    };
    std::fs::create_dir_all(&dir).expect("prepare the unit dir");
    let suffix = match kind {
        UnitKind::Launchd => "plist",
        UnitKind::Systemd => "service",
        UnitKind::WinSchtasks => "xml",
    };
    let path = dir.join(format!("pm3-fixture.{suffix}"));
    std::fs::write(&path, "old unit").expect("write the old unit");
    path
}

fn seed_dump(fixture: &Fixture, name: &str, pid: Option<u32>) {
    let pid = pid.map_or_else(|| "null".to_string(), |pid| pid.to_string());
    let body = format!(
        "services:\n- name: {name}\n  runtime:\n    pm_id: 0\n    status: online\n    restart_time: 0\n    unstable_restarts: 0\n    created_at_ms: 1\n    pid: {pid}\n    started_at_ms: 2\n"
    );
    std::fs::write(fixture.home.join("dump.yaml"), body).expect("write the dump");
}

fn health_server(socket: PathBuf, refuse_first: u32) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let listener = tokio::net::UnixListener::bind(socket).expect("bind the fake daemon socket");
        let mut seen = 0u32;
        while let Ok((mut stream, _)) = listener.accept().await {
            seen += 1;
            let status = if seen <= refuse_first {
                "500 no"
            } else {
                "200 OK"
            };
            let body = "OK";
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let mut request = [0u8; 1024];
            let _ = stream.read(&mut request).await;
            stream.write_all(response.as_bytes()).await.ok();
        }
    })
}

fn stamp_dir(fixture: &Fixture) -> PathBuf {
    fixture.backups.join("unknown")
}

const HEALTHY_SYSTEMD: &str =
    "case \"$2\" in\n  is-active) echo active ;;\n  show) echo 4242 ;;\nesac\nexit 0";

const HEALTHY_SCHTASKS: &str = "case \"$1\" in\n  /Query) echo 'Status: Running' ;;\nesac\nexit 0";

#[tokio::test]
async fn a_schtasks_install_treats_the_pid_file_as_the_supervised_pid() {
    let fixture = systemd_fixture(HEALTHY_SCHTASKS);
    let source = seed_source(&fixture);
    seed_pid_file(&fixture);
    let server = health_server(fixture.home.join("pm3.sock"), 1);
    let lines = std::sync::Mutex::new(Vec::new());
    let emit = |line: &str| lines.lock().expect("lock").push(line.to_string());

    run_install(
        &fixture.config_path,
        Some(source),
        &context(&fixture, UnitKind::WinSchtasks, None),
        &emit,
    )
    .await
    .expect("the install should succeed");
    server.abort();

    let service_dir = fixture.home.join(".pm3/service");
    assert!(
        service_dir.join("pm3-fixture.xml").is_file(),
        "the task xml should exist"
    );
    assert!(
        service_dir.join("pm3-fixture-daemon.cmd").is_file(),
        "the wrapper should exist"
    );
    let output = lines.lock().expect("lock").join("\n");
    assert!(
        output.contains("service pm3-fixture (schtasks) is running"),
        "got: {output}"
    );
}

#[tokio::test]
async fn a_first_install_swaps_the_binary_and_verifies_the_takeover() {
    let fixture = systemd_fixture(HEALTHY_SYSTEMD);
    let source = seed_source(&fixture);
    seed_pid_file(&fixture);
    let server = health_server(fixture.home.join("pm3.sock"), 1);
    let lines = std::sync::Mutex::new(Vec::new());
    let emit = |line: &str| lines.lock().expect("lock").push(line.to_string());

    run_install(
        &fixture.config_path,
        Some(source),
        &context(&fixture, UnitKind::Systemd, None),
        &emit,
    )
    .await
    .expect("the install should succeed");
    server.abort();

    let installed = std::fs::read_to_string(&fixture.destination).expect("read the destination");
    assert_eq!(installed, "new binary");
    assert!(stamp_dir(&fixture).is_dir(), "the stamp dir is created");
    let output = lines.lock().expect("lock").join("\n");
    assert!(output.contains("backed up"), "got: {output}");
    assert!(
        output.contains("service pm3-fixture (systemd) is running"),
        "got: {output}"
    );
    assert!(
        output.contains("no managed services to reclaim"),
        "got: {output}"
    );
}

#[tokio::test]
async fn an_upgrade_backs_up_the_binary_the_config_and_the_unit_under_its_version() {
    let launchd = "case \"$1\" in\n  list) echo '\"PID\" = 4242;' ;;\nesac\nexit 0";
    let fixture = systemd_fixture(launchd);
    let source = seed_source(&fixture);
    seed_pid_file(&fixture);
    seed_dump(&fixture, "api", Some(MANAGER_PID));
    std::fs::create_dir_all(fixture.destination.parent().expect("the dest dir"))
        .expect("prepare the dest dir");
    std::fs::write(&fixture.destination, "#!/bin/sh\necho 'pm3 9.9.9'\n")
        .expect("write the old binary");
    std::fs::set_permissions(&fixture.destination, std::fs::Permissions::from_mode(0o755))
        .expect("chmod the old binary");
    std::fs::write(fixture.home.join("config.yaml"), "old config").expect("write the old config");
    unit_file(&fixture, UnitKind::Launchd);
    let server = health_server(fixture.home.join("pm3.sock"), 1);
    let lines = std::sync::Mutex::new(Vec::new());
    let emit = |line: &str| lines.lock().expect("lock").push(line.to_string());

    run_install(
        &fixture.config_path,
        Some(source),
        &context(&fixture, UnitKind::Launchd, Some(4242)),
        &emit,
    )
    .await
    .expect("the install should succeed");
    server.abort();

    let stamp = fixture.backups.join("9.9.9");
    assert!(stamp.join("pm3").is_file(), "the old binary is backed up");
    assert!(
        stamp.join("config.yaml").is_file(),
        "the old config is backed up"
    );
    assert!(
        stamp.join("pm3-fixture.plist").is_file(),
        "the old unit is backed up"
    );
    let output = lines.lock().expect("lock").join("\n");
    assert!(output.contains("adopted 1: api"), "got: {output}");
    assert!(output.contains("backups/9.9.9"), "got: {output}");
}

#[tokio::test]
async fn an_install_without_a_source_uses_the_running_binary() {
    let fixture = systemd_fixture(HEALTHY_SYSTEMD);
    let source = seed_source(&fixture);
    seed_pid_file(&fixture);
    let server = health_server(fixture.home.join("pm3.sock"), 1);
    let emit = |_line: &str| {};

    run_install(
        &fixture.config_path,
        None,
        &context(&fixture, UnitKind::Systemd, None),
        &emit,
    )
    .await
    .expect("the running binary becomes the source");
    server.abort();
    assert_eq!(
        std::fs::read_to_string(&fixture.destination).expect("read the destination"),
        std::fs::read_to_string(source).expect("read the source")
    );
}

#[tokio::test]
async fn an_install_reports_a_source_it_cannot_name() {
    let fixture = systemd_fixture(HEALTHY_SYSTEMD);
    let emit = |_line: &str| {};
    let broken = io::Error::new(io::ErrorKind::NotFound, "no executable path");
    let error = run_install(
        &fixture.config_path,
        None,
        &context_with_exe(&fixture, UnitKind::Systemd, None, Err(broken)),
        &emit,
    )
    .await
    .unwrap_err();
    assert!(
        error
            .to_string()
            .starts_with("cannot determine the pm3 binary path"),
        "got: {error}"
    );
}

#[tokio::test]
async fn an_install_without_a_home_cannot_find_a_destination() {
    let fixture = systemd_fixture(HEALTHY_SYSTEMD);
    let emit = |_line: &str| {};
    let mut context = context(&fixture, UnitKind::Systemd, None);
    context.home_env = None;
    context.destination_env = None;
    let error = run_install(&fixture.config_path, None, &context, &emit)
        .await
        .unwrap_err();
    assert!(
        error.to_string().contains("no HOME in the environment"),
        "got: {error}"
    );
}

#[tokio::test]
async fn an_install_reports_a_config_it_cannot_load() {
    let fixture = systemd_fixture(HEALTHY_SYSTEMD);
    let emit = |_line: &str| {};
    let error = run_install(
        "/nonexistent/pm3-config.yaml",
        None,
        &context(&fixture, UnitKind::Systemd, None),
        &emit,
    )
    .await
    .unwrap_err();
    assert!(
        error.to_string().contains("cannot resolve the config path"),
        "got: {error}"
    );
}

#[tokio::test]
async fn an_install_reports_a_corrupt_dump_before_touching_anything() {
    let fixture = systemd_fixture(HEALTHY_SYSTEMD);
    seed_source(&fixture);
    std::fs::write(fixture.home.join("dump.yaml"), "not: [yaml").expect("write a corrupt dump");
    let emit = |_line: &str| {};
    let error = run_install(
        &fixture.config_path,
        None,
        &context(&fixture, UnitKind::Systemd, None),
        &emit,
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("cannot read"), "got: {error}");
    assert!(!fixture.destination.exists(), "the binary is untouched");
}

#[tokio::test]
async fn an_install_reports_a_binary_it_cannot_back_up() {
    let fixture = systemd_fixture(HEALTHY_SYSTEMD);
    seed_source(&fixture);
    std::fs::create_dir_all(&fixture.destination).expect("a directory occupies the destination");
    let emit = |_line: &str| {};
    let error = run_install(
        &fixture.config_path,
        None,
        &context(&fixture, UnitKind::Systemd, None),
        &emit,
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("cannot back up"), "got: {error}");
}

#[tokio::test]
async fn an_install_reports_a_source_it_cannot_read() {
    let fixture = systemd_fixture(HEALTHY_SYSTEMD);
    let emit = |_line: &str| {};
    let error = run_install(
        &fixture.config_path,
        Some(fixture.dir.path().join("missing")),
        &context(&fixture, UnitKind::Systemd, None),
        &emit,
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("cannot replace"), "got: {error}");
}

#[tokio::test]
async fn an_install_reports_a_unit_it_cannot_back_up() {
    let fixture = systemd_fixture(HEALTHY_SYSTEMD);
    seed_source(&fixture);
    std::fs::create_dir_all(fixture.home.join("config.yaml"))
        .expect("a directory occupies the settled config");
    let emit = |_line: &str| {};
    let error = run_install(
        &fixture.config_path,
        None,
        &context(&fixture, UnitKind::Systemd, None),
        &emit,
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("cannot back up"), "got: {error}");
}

#[tokio::test]
async fn an_install_reports_a_failed_uninstall() {
    let fixture = systemd_fixture(HEALTHY_SYSTEMD);
    seed_source(&fixture);
    let unit = unit_file(&fixture, UnitKind::Systemd);
    let dir = unit.parent().expect("the unit dir");
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o500)).expect("lock the dir");
    let emit = |_line: &str| {};
    let outcome = run_install(
        &fixture.config_path,
        None,
        &context(&fixture, UnitKind::Systemd, None),
        &emit,
    )
    .await;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700)).expect("unlock the dir");
    let error = outcome.unwrap_err();
    assert!(error.to_string().contains("cannot"), "got: {error}");
}

#[tokio::test]
async fn an_install_reports_a_daemon_whose_pid_is_unknown() {
    let fixture = systemd_fixture(HEALTHY_SYSTEMD);
    seed_source(&fixture);
    let server = health_server(fixture.home.join("pm3.sock"), 0);
    let emit = |_line: &str| {};
    let error = run_install(
        &fixture.config_path,
        None,
        &context(&fixture, UnitKind::Systemd, None),
        &emit,
    )
    .await
    .unwrap_err();
    server.abort();
    assert!(
        error.to_string().contains("cannot read the pm3 daemon pid"),
        "got: {error}"
    );
}

#[tokio::test]
async fn an_install_reports_a_failed_reinstall() {
    let fixture = systemd_fixture("case \"$2\" in\n  enable) exit 1 ;;\nesac\nexit 0");
    seed_source(&fixture);
    let emit = |_line: &str| {};
    let error = run_install(
        &fixture.config_path,
        None,
        &context(&fixture, UnitKind::Systemd, None),
        &emit,
    )
    .await
    .unwrap_err();
    assert!(
        error.to_string().contains("cannot complete"),
        "got: {error}"
    );
}

#[tokio::test]
async fn an_install_reports_a_takeover_that_never_happens() {
    let fixture = impatient_systemd_fixture(
        "case \"$2\" in\n  is-active) echo active ;;\n  show) echo 1 ;;\nesac\nexit 0",
    );
    seed_source(&fixture);
    seed_pid_file(&fixture);
    let server = health_server(fixture.home.join("pm3.sock"), 1);
    let emit = |_line: &str| {};
    let error = run_install(
        &fixture.config_path,
        None,
        &context(&fixture, UnitKind::Systemd, None),
        &emit,
    )
    .await
    .unwrap_err();
    server.abort();
    let message = error.to_string();
    assert!(message.contains("did not come under"), "got: {message}");
    assert!(message.contains("backups/unknown"), "got: {message}");
}

#[tokio::test]
async fn a_launchd_install_recovers_via_a_kickstart() {
    let launchd = "case \"$1\" in\n  list) if [ -f \"$0.kicked\" ]; then echo '\"PID\" = 4242;'; else echo '{}' ; fi ;;\n  kickstart) touch \"$0.kicked\" ;;\nesac\nexit 0";
    let fixture = impatient_systemd_fixture(launchd);
    seed_source(&fixture);
    seed_pid_file(&fixture);
    let server = health_server(fixture.home.join("pm3.sock"), 1);
    let emit = |_line: &str| {};
    run_install(
        &fixture.config_path,
        None,
        &context(&fixture, UnitKind::Launchd, Some(4242)),
        &emit,
    )
    .await
    .expect("a kickstart should recover the takeover");
    server.abort();
    let kicked = format!("{}.kicked", fixture.programs.launchctl);
    assert!(Path::new(&kicked).exists(), "the agent was kicked");
}

#[tokio::test]
async fn a_launchd_install_reports_a_failed_kickstart() {
    let launchd = "case \"$1\" in\n  list) echo '{}' ;;\n  kickstart) exit 1 ;;\nesac\nexit 0";
    let fixture = impatient_systemd_fixture(launchd);
    seed_source(&fixture);
    seed_pid_file(&fixture);
    let server = health_server(fixture.home.join("pm3.sock"), 1);
    let emit = |_line: &str| {};
    let error = run_install(
        &fixture.config_path,
        None,
        &context(&fixture, UnitKind::Launchd, Some(4242)),
        &emit,
    )
    .await
    .unwrap_err();
    server.abort();
    assert!(
        error.to_string().contains("cannot complete"),
        "got: {error}"
    );
}

#[tokio::test]
async fn a_takeover_probe_reports_a_manager_it_cannot_run() {
    let launchd = "case \"$1\" in\n  load) rm \"$0\" ;;\nesac\nexit 0";
    let fixture = systemd_fixture(launchd);
    seed_source(&fixture);
    seed_pid_file(&fixture);
    let server = health_server(fixture.home.join("pm3.sock"), 1);
    let emit = |_line: &str| {};
    let error = run_install(
        &fixture.config_path,
        None,
        &context(&fixture, UnitKind::Launchd, Some(4242)),
        &emit,
    )
    .await
    .unwrap_err();
    server.abort();
    assert!(error.to_string().contains("cannot run"), "got: {error}");
}

#[tokio::test]
async fn a_takeover_probe_reports_a_pid_query_it_cannot_run() {
    let systemd = "case \"$2\" in\n  is-active) echo active ;;\n  show) rm \"$0\" ;;\nesac\nexit 0";
    let fixture = systemd_fixture(systemd);
    seed_source(&fixture);
    seed_pid_file(&fixture);
    let server = health_server(fixture.home.join("pm3.sock"), 1);
    let emit = |_line: &str| {};
    let error = run_install(
        &fixture.config_path,
        None,
        &context(&fixture, UnitKind::Systemd, None),
        &emit,
    )
    .await
    .unwrap_err();
    server.abort();
    assert!(error.to_string().contains("cannot run"), "got: {error}");
}

#[tokio::test]
async fn a_takeover_probe_reports_a_pid_query_that_stalls() {
    let systemd = "case \"$2\" in\n  is-active) echo active ;;\n  show) sleep 30 ;;\nesac\nexit 0";
    let fixture = systemd_fixture(systemd);
    let yaml = std::fs::read_to_string(&fixture.config_path).expect("read the config");
    let patched = yaml.replace("command_timeout_ms: 5000", "command_timeout_ms: 40");
    assert!(
        patched.contains("command_timeout_ms: 40"),
        "the timeout is patched"
    );
    std::fs::write(&fixture.config_path, patched).expect("patch the config");
    seed_source(&fixture);
    seed_pid_file(&fixture);
    let server = health_server(fixture.home.join("pm3.sock"), 1);
    let emit = |_line: &str| {};
    let error = run_install(
        &fixture.config_path,
        None,
        &context(&fixture, UnitKind::Systemd, None),
        &emit,
    )
    .await
    .unwrap_err();
    server.abort();
    assert!(
        error.to_string().contains("cannot get an answer"),
        "got: {error}"
    );
}

#[tokio::test]
async fn a_launchd_install_reports_a_manager_lost_after_the_kickstart() {
    let launchd = "case \"$1\" in\n  list) echo '{}' ;;\n  kickstart) rm \"$0\" ;;\nesac\nexit 0";
    let fixture = impatient_systemd_fixture(launchd);
    seed_source(&fixture);
    seed_pid_file(&fixture);
    let server = health_server(fixture.home.join("pm3.sock"), 1);
    let emit = |_line: &str| {};
    let error = run_install(
        &fixture.config_path,
        None,
        &context(&fixture, UnitKind::Launchd, Some(4242)),
        &emit,
    )
    .await
    .unwrap_err();
    server.abort();
    assert!(error.to_string().contains("cannot run"), "got: {error}");
}

#[tokio::test]
async fn an_install_reports_a_dump_it_cannot_reread() {
    let systemd = "case \"$2\" in\n  is-active) echo active ;;\n  show) echo 4242 ;;\n  enable) echo 'not: [yaml' > \"$PM3_DUMP\" ;;\nesac\nexit 0";
    let fixture = systemd_fixture(systemd);
    seed_source(&fixture);
    seed_pid_file(&fixture);
    seed_dump(&fixture, "api", Some(MANAGER_PID));
    let dump = fixture.home.join("dump.yaml");
    let manager = fixture.programs.systemctl.clone();
    std::fs::write(
        &manager,
        format!("#!/bin/sh\nPM3_DUMP={}\n{systemd}\n", dump.display()),
    )
    .expect("rewrite the fake manager");
    std::fs::set_permissions(&manager, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let server = health_server(fixture.home.join("pm3.sock"), 1);
    let emit = |_line: &str| {};
    let error = run_install(
        &fixture.config_path,
        None,
        &context(&fixture, UnitKind::Systemd, None),
        &emit,
    )
    .await
    .unwrap_err();
    server.abort();
    assert!(error.to_string().contains("cannot read"), "got: {error}");
}

#[tokio::test]
async fn an_install_reports_services_it_lost() {
    let systemd = "case \"$2\" in\n  is-active) echo active ;;\n  show) echo 4242 ;;\n  enable) printf 'services: []\\n' > \"$PM3_DUMP\" ;;\nesac\nexit 0";
    let fixture = systemd_fixture(systemd);
    seed_source(&fixture);
    seed_pid_file(&fixture);
    seed_dump(&fixture, "api", Some(MANAGER_PID));
    let dump = fixture.home.join("dump.yaml");
    let manager = fixture.programs.systemctl.clone();
    std::fs::write(
        &manager,
        format!("#!/bin/sh\nPM3_DUMP={}\n{systemd}\n", dump.display()),
    )
    .expect("rewrite the fake manager");
    std::fs::set_permissions(&manager, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let server = health_server(fixture.home.join("pm3.sock"), 1);
    let lines = std::sync::Mutex::new(Vec::new());
    let emit = |line: &str| lines.lock().expect("lock").push(line.to_string());
    let error = run_install(
        &fixture.config_path,
        None,
        &context(&fixture, UnitKind::Systemd, None),
        &emit,
    )
    .await
    .unwrap_err();
    server.abort();
    let message = error.to_string();
    assert!(
        message.contains("not every managed service came back"),
        "got: {message}"
    );
    assert!(message.contains("lost 1: api"), "got: {message}");
    let output = lines.lock().expect("lock").join("\n");
    assert!(output.contains("lost 1: api"), "got: {output}");
}

#[test]
fn a_takeover_needs_a_running_supervised_and_healthy_daemon() {
    assert!(takeover_satisfied(
        UnitStatus::Running,
        Some(4242),
        Some(4242),
        true
    ));
    assert!(!takeover_satisfied(
        UnitStatus::InstalledNotRunning,
        Some(4242),
        Some(4242),
        true
    ));
    assert!(!takeover_satisfied(
        UnitStatus::Running,
        None,
        Some(4242),
        true
    ));
    assert!(!takeover_satisfied(
        UnitStatus::Running,
        Some(1),
        Some(4242),
        true
    ));
    assert!(!takeover_satisfied(
        UnitStatus::Running,
        Some(4242),
        Some(4242),
        false
    ));
}
