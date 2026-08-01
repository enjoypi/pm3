use std::{fs, os::unix::fs::PermissionsExt as _};

use usecases::{LaunchSpec, ProcessLauncher as _};

use super::*;

const POLL_MS: u64 = 10;
const CADENCE: PollCadence = PollCadence {
    interval_ms: POLL_MS,
    max_interval_ms: POLL_MS,
};
const ADOPTED_PID: u32 = 4242;
const FIXTURE_TOKEN: &str = "Tue Jul 28 14:06:28 2026";

struct Fixture {
    dir: tempfile::TempDir,
    probe: PsProcessProbe,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().expect("temp dir");
    let alive = dir.path().join("alive");
    let broken = dir.path().join("broken");
    let script = dir.path().join("ps");
    fs::write(
        &script,
        format!(
            "#!/bin/sh\nif [ -f {} ]; then exit 2; fi\nif [ -f {} ]; then echo '{FIXTURE_TOKEN}'; else exit 1; fi\n",
            broken.display(),
            alive.display()
        ),
    )
    .expect("should write a fake ps");
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755))
        .expect("should make the fake ps executable");
    let probe = PsProcessProbe::with_program(script.to_string_lossy().into_owned());
    Fixture { dir, probe }
}

impl Fixture {
    fn mark_alive(&self) {
        fs::write(self.dir.path().join("alive"), b"").expect("should mark the process alive");
    }

    fn mark_gone(&self) {
        fs::remove_file(self.dir.path().join("alive")).expect("should mark the process gone");
    }

    fn make_probe_unreadable(&self) {
        fs::write(self.dir.path().join("broken"), b"").expect("should break the fake ps");
    }

    fn make_probe_readable(&self) {
        fs::remove_file(self.dir.path().join("broken")).expect("should repair the fake ps");
    }
}

fn launch_spec(dir: &tempfile::TempDir) -> LaunchSpec {
    LaunchSpec {
        name: "api".to_string(),
        program: "/usr/bin/true".to_string(),
        args: Vec::new(),
        cwd: "/".to_string(),
        env: Vec::new(),
        stdout_path: dir.path().join("out.log").to_string_lossy().into_owned(),
        stderr_path: dir.path().join("err.log").to_string_lossy().into_owned(),
    }
}

#[tokio::test]
async fn a_real_child_is_reaped_through_its_own_handle() {
    let fixture = fixture();
    let launcher = TokioProcessLauncher::default();
    let child = launcher
        .spawn(&launch_spec(&fixture.dir))
        .await
        .expect("should spawn /usr/bin/true");
    let outcome = wait_for_exit(&launcher, &fixture.probe, child.pid, None, CADENCE)
        .await
        .expect("a real child reports an exit");
    assert_eq!(outcome.exit_code, Some(0));
}

#[tokio::test]
async fn an_adopted_process_that_already_left_is_reported_at_once() {
    let fixture = fixture();
    let launcher = TokioProcessLauncher::default();
    launcher.adopt(ADOPTED_PID).await;
    let outcome = wait_for_exit(&launcher, &fixture.probe, ADOPTED_PID, None, CADENCE)
        .await
        .expect("an adopted process reports an exit");
    assert_eq!(outcome.exit_code, None);
}

#[tokio::test]
async fn a_pid_the_kernel_handed_to_someone_else_counts_as_an_exit() {
    let fixture = fixture();
    fixture.mark_alive();
    let launcher = TokioProcessLauncher::default();
    launcher.adopt(ADOPTED_PID).await;
    let outcome = wait_for_exit(
        &launcher,
        &fixture.probe,
        ADOPTED_PID,
        Some("Mon Jan 01 00:00:00 2020".to_string()),
        CADENCE,
    )
    .await
    .expect("a recycled pid reports an exit");
    assert_eq!(outcome.exit_code, None);
}

#[tokio::test]
async fn a_pid_still_holding_the_recorded_identity_keeps_being_watched() {
    let fixture = fixture();
    fixture.mark_alive();
    let launcher = TokioProcessLauncher::default();
    launcher.adopt(ADOPTED_PID).await;

    let observer = wait_for_exit(
        &launcher,
        &fixture.probe,
        ADOPTED_PID,
        Some(FIXTURE_TOKEN.to_string()),
        CADENCE,
    );
    let reaper = async {
        tokio::time::sleep(std::time::Duration::from_millis(POLL_MS * 3)).await;
        fixture.mark_gone();
    };
    let (outcome, ()) = tokio::join!(observer, reaper);
    assert_eq!(outcome.expect("the adopted process left").exit_code, None);
}

#[tokio::test]
async fn a_probe_that_cannot_answer_keeps_the_process_under_watch() {
    let fixture = fixture();
    fixture.mark_alive();
    fixture.make_probe_unreadable();
    let launcher = TokioProcessLauncher::default();
    launcher.adopt(ADOPTED_PID).await;

    let observer = wait_for_exit(&launcher, &fixture.probe, ADOPTED_PID, None, CADENCE);
    let repairer = async {
        tokio::time::sleep(std::time::Duration::from_millis(POLL_MS * 3)).await;
        fixture.mark_gone();
        fixture.make_probe_readable();
    };
    let (outcome, ()) = tokio::join!(observer, repairer);
    assert_eq!(outcome.expect("the adopted process left").exit_code, None);
}

#[tokio::test]
async fn an_adopted_process_stops_being_tracked_once_it_leaves() {
    let fixture = fixture();
    let launcher = TokioProcessLauncher::default();
    launcher.adopt(ADOPTED_PID).await;
    assert_eq!(launcher.tracked_pids().await, vec![ADOPTED_PID]);
    wait_for_exit(&launcher, &fixture.probe, ADOPTED_PID, None, CADENCE).await;
    assert!(launcher.tracked_pids().await.is_empty());
}

#[tokio::test]
async fn an_adopted_process_is_polled_until_it_leaves() {
    let fixture = fixture();
    fixture.mark_alive();
    let launcher = TokioProcessLauncher::default();
    launcher.adopt(ADOPTED_PID).await;

    let observer = wait_for_exit(&launcher, &fixture.probe, ADOPTED_PID, None, CADENCE);
    let reaper = async {
        tokio::time::sleep(std::time::Duration::from_millis(POLL_MS * 3)).await;
        fixture.mark_gone();
    };
    let (outcome, ()) = tokio::join!(observer, reaper);
    assert_eq!(outcome.expect("the adopted process left").exit_code, None);
}

#[test]
fn the_poll_interval_doubles_until_it_reaches_its_ceiling() {
    let cadence = PollCadence {
        interval_ms: 50,
        max_interval_ms: 1000,
    };
    assert_eq!(cadence.next_after(50), 100);
    assert_eq!(cadence.next_after(400), 800);
}

#[test]
fn the_poll_interval_never_passes_its_ceiling() {
    let cadence = PollCadence {
        interval_ms: 50,
        max_interval_ms: 1000,
    };
    assert_eq!(cadence.next_after(800), 1000);
    assert_eq!(cadence.next_after(1000), 1000);
}

#[tokio::test]
async fn a_path_that_is_already_gone_is_released_at_once() {
    let dir = tempfile::tempdir().expect("temp dir");
    assert!(wait_until_released(&dir.path().join("pm3.sock"), 60_000, POLL_MS).await);
}

#[tokio::test]
async fn a_path_that_never_goes_away_exhausts_the_budget() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("pm3.sock");
    fs::write(&socket, b"socket").expect("seed the socket");
    assert!(!wait_until_released(&socket, 0, POLL_MS).await);
}

#[tokio::test]
async fn a_path_removed_while_waiting_is_released() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("pm3.sock");
    fs::write(&socket, b"socket").expect("seed the socket");
    let waiting = wait_until_released(&socket, 60_000, POLL_MS);
    let remover = async {
        tokio::time::sleep(std::time::Duration::from_millis(POLL_MS * 2)).await;
        fs::remove_file(&socket).expect("release the socket");
    };
    let (released, ()) = tokio::join!(waiting, remover);
    assert!(released);
}
