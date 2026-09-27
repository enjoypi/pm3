#[cfg(unix)]
use super::manager::{KICKSTART_DELETES_ITSELF, LOAD_DELETES_ITSELF, SHOW_DELETES_ITSELF};
use super::{
    manager::{
        ENABLE_CORRUPTS_DUMP, ENABLE_EMPTIES_DUMP, FOREIGN_PID, KICKSTART_RECOVERS,
        KICKSTART_REFUSED, REFUSING_ENABLE, SHOW_STALLS, write_manager,
    },
    *,
};

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

#[cfg(unix)]
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
    let server = health_server(fixture.home.join("pm3.sock"), 0).await;
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
    let fixture = systemd_fixture(REFUSING_ENABLE);
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
    let fixture = impatient_systemd_fixture(FOREIGN_PID);
    seed_source(&fixture);
    seed_pid_file(&fixture);
    let server = health_server(fixture.home.join("pm3.sock"), 1).await;
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
    let backup = stamp_dir(&fixture).to_string_lossy().into_owned();
    assert!(message.contains(&backup), "got: {message}");
}

#[tokio::test]
async fn a_launchd_install_recovers_via_a_kickstart() {
    let fixture = impatient_systemd_fixture(KICKSTART_RECOVERS);
    seed_source(&fixture);
    seed_pid_file(&fixture);
    let server = health_server(fixture.home.join("pm3.sock"), 1).await;
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
    let fixture = impatient_systemd_fixture(KICKSTART_REFUSED);
    seed_source(&fixture);
    seed_pid_file(&fixture);
    let server = health_server(fixture.home.join("pm3.sock"), 1).await;
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

#[cfg(unix)]
#[tokio::test]
async fn a_takeover_probe_reports_a_manager_it_cannot_run() {
    let fixture = systemd_fixture(LOAD_DELETES_ITSELF);
    seed_source(&fixture);
    seed_pid_file(&fixture);
    let server = health_server(fixture.home.join("pm3.sock"), 1).await;
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

#[cfg(unix)]
#[tokio::test]
async fn a_takeover_probe_reports_a_pid_query_it_cannot_run() {
    let fixture = systemd_fixture(SHOW_DELETES_ITSELF);
    seed_source(&fixture);
    seed_pid_file(&fixture);
    let server = health_server(fixture.home.join("pm3.sock"), 1).await;
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
    let fixture = systemd_fixture(SHOW_STALLS);
    let yaml = std::fs::read_to_string(&fixture.config_path).expect("read the config");
    let patched = yaml.replace("command_timeout_ms: 5000", "command_timeout_ms: 40");
    assert!(
        patched.contains("command_timeout_ms: 40"),
        "the timeout is patched"
    );
    std::fs::write(&fixture.config_path, patched).expect("patch the config");
    seed_source(&fixture);
    seed_pid_file(&fixture);
    let server = health_server(fixture.home.join("pm3.sock"), 1).await;
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

#[cfg(unix)]
#[tokio::test]
async fn a_launchd_install_reports_a_manager_lost_after_the_kickstart() {
    let fixture = impatient_systemd_fixture(KICKSTART_DELETES_ITSELF);
    seed_source(&fixture);
    seed_pid_file(&fixture);
    let server = health_server(fixture.home.join("pm3.sock"), 1).await;
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
    let fixture = systemd_fixture(ENABLE_CORRUPTS_DUMP);
    seed_source(&fixture);
    seed_pid_file(&fixture);
    seed_dump(&fixture, "api", Some(MANAGER_PID));
    let dump = fixture.home.join("dump.yaml");
    let manager = fixture.programs.systemctl.clone();
    write_manager(Path::new(&manager), Some(&dump), ENABLE_CORRUPTS_DUMP);
    let server = health_server(fixture.home.join("pm3.sock"), 1).await;
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
    let fixture = systemd_fixture(ENABLE_EMPTIES_DUMP);
    seed_source(&fixture);
    seed_pid_file(&fixture);
    seed_dump(&fixture, "api", Some(MANAGER_PID));
    let dump = fixture.home.join("dump.yaml");
    let manager = fixture.programs.systemctl.clone();
    write_manager(Path::new(&manager), Some(&dump), ENABLE_EMPTIES_DUMP);
    let server = health_server(fixture.home.join("pm3.sock"), 1).await;
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
