use std::{fs, os::unix::fs::PermissionsExt as _};

use usecases::{LaunchSpec, ProcessLauncher as _};

use super::*;

const POLL_MS: u64 = 10;
const ADOPTED_PID: u32 = 4242;

struct Fixture {
    dir: tempfile::TempDir,
    probe: PsProcessProbe,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().expect("temp dir");
    let alive = dir.path().join("alive");
    let script = dir.path().join("ps");
    fs::write(
        &script,
        format!(
            "#!/bin/sh\nif [ -f {} ]; then echo 'Tue Jul 28 14:06:28 2026'; else exit 1; fi\n",
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
    let outcome = wait_for_exit(&launcher, &fixture.probe, child.pid, POLL_MS)
        .await
        .expect("a real child reports an exit");
    assert_eq!(outcome.exit_code, Some(0));
}

#[tokio::test]
async fn an_adopted_process_that_already_left_is_reported_at_once() {
    let fixture = fixture();
    let launcher = TokioProcessLauncher::default();
    launcher.adopt(ADOPTED_PID).await;
    let outcome = wait_for_exit(&launcher, &fixture.probe, ADOPTED_PID, POLL_MS)
        .await
        .expect("an adopted process reports an exit");
    assert_eq!(outcome.exit_code, None);
}

#[tokio::test]
async fn an_adopted_process_stops_being_tracked_once_it_leaves() {
    let fixture = fixture();
    let launcher = TokioProcessLauncher::default();
    launcher.adopt(ADOPTED_PID).await;
    assert_eq!(launcher.tracked_pids().await, vec![ADOPTED_PID]);
    wait_for_exit(&launcher, &fixture.probe, ADOPTED_PID, POLL_MS).await;
    assert!(launcher.tracked_pids().await.is_empty());
}

#[tokio::test]
async fn an_adopted_process_is_polled_until_it_leaves() {
    let fixture = fixture();
    fixture.mark_alive();
    let launcher = TokioProcessLauncher::default();
    launcher.adopt(ADOPTED_PID).await;

    let observer = wait_for_exit(&launcher, &fixture.probe, ADOPTED_PID, POLL_MS);
    let reaper = async {
        tokio::time::sleep(std::time::Duration::from_millis(POLL_MS * 3)).await;
        fixture.mark_gone();
    };
    let (outcome, ()) = tokio::join!(observer, reaper);
    assert_eq!(outcome.expect("the adopted process left").exit_code, None);
}

#[tokio::test]
async fn a_path_that_is_already_gone_is_released_at_once() {
    let dir = tempfile::tempdir().expect("temp dir");
    assert!(wait_until_released(&dir.path().join("pm3.sock"), 60_000).await);
}

#[tokio::test]
async fn a_path_that_never_goes_away_exhausts_the_budget() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("pm3.sock");
    fs::write(&socket, b"socket").expect("seed the socket");
    assert!(!wait_until_released(&socket, 0).await);
}

#[tokio::test]
async fn a_path_removed_while_waiting_is_released() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("pm3.sock");
    fs::write(&socket, b"socket").expect("seed the socket");
    let waiting = wait_until_released(&socket, 60_000);
    let remover = async {
        tokio::time::sleep(std::time::Duration::from_millis(
            RELEASE_POLL_INTERVAL_MS * 2,
        ))
        .await;
        fs::remove_file(&socket).expect("release the socket");
    };
    let (released, ()) = tokio::join!(waiting, remover);
    assert!(released);
}
