#![cfg(unix)]
use std::{
    collections::HashMap,
    fs,
    os::unix::fs::PermissionsExt as _,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use tokio::sync::oneshot;
use usecases::{LaunchSpec, ProcessLauncher as _};

use super::*;

const POLL_STEP_MS: u64 = 20;

const POLL_MS: u64 = 10;
const PROBE_TIMEOUT_MS: u64 = 5000;
const CADENCE: PollCadence = PollCadence {
    interval_ms: POLL_MS,
    max_interval_ms: POLL_MS,
};
const ADOPTED_PID: u32 = 4242;
const FIXTURE_TOKEN: &str = "Tue Jul 28 14:06:28 2026";

struct Fixture {
    dir: tempfile::TempDir,
    probe: Arc<PsProcessProbe>,
    watch: Arc<AdoptedWatch>,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().expect("temp dir");
    let alive = dir.path().join("alive");
    let script = dir.path().join("ps");
    fs::write(
        &script,
        format!(
            concat!(
                "#!/bin/sh\n",
                "if [ ! -f {} ]; then exit 1; fi\n",
                "for pid in $(echo \"$5\" | tr ',' ' '); do echo \"$pid {}\"; done\n",
            ),
            alive.display(),
            FIXTURE_TOKEN,
        ),
    )
    .expect("should write a fake ps");
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755))
        .expect("should make the fake ps executable");
    let probe = Arc::new(PsProcessProbe::new(
        script.to_string_lossy().into_owned(),
        PROBE_TIMEOUT_MS,
        POLL_STEP_MS,
    ));
    Fixture {
        dir,
        probe,
        watch: Arc::new(AdoptedWatch::default()),
    }
}

fn fixture_that_answers_once(first_answer: &str) -> Fixture {
    let dir = tempfile::tempdir().expect("temp dir");
    let script = dir.path().join("ps");
    fs::write(
        &script,
        format!(
            concat!(
                "#!/bin/sh\n",
                "if [ -f \"$0.asked\" ]; then exit 1; fi\n",
                "touch \"$0.asked\"\n",
                "{}\n",
            ),
            first_answer,
        ),
    )
    .expect("should write a fake ps");
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755))
        .expect("should make the fake ps executable");
    Fixture {
        dir,
        probe: Arc::new(PsProcessProbe::new(
            script.to_string_lossy().into_owned(),
            PROBE_TIMEOUT_MS,
            POLL_STEP_MS,
        )),
        watch: Arc::new(AdoptedWatch::default()),
    }
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
async fn a_waiter_registered_after_the_snapshot_survives_the_release() {
    let watch = AdoptedWatch::default();
    let (departed, mut gone) = oneshot::channel();
    watch.state.lock().await.watched.insert(
        ADOPTED_PID,
        Watched {
            waiters: vec![Waiter {
                token: Some(FIXTURE_TOKEN.to_string()),
                departed,
            }],
        },
    );
    let seen = HashMap::new();
    watch.release(&seen).await;
    assert!(
        watch.state.lock().await.watched.contains_key(&ADOPTED_PID),
        "a pid the latest ps snapshot did not cover must stay under watch"
    );
    assert!(matches!(
        gone.try_recv(),
        Err(oneshot::error::TryRecvError::Empty)
    ));
}

#[tokio::test]
async fn a_real_child_is_reaped_through_its_own_handle() {
    let fixture = fixture();
    let launcher = TokioProcessLauncher::default();
    let child = launcher
        .spawn(&launch_spec(&fixture.dir))
        .await
        .expect("should spawn /usr/bin/true");
    let outcome = wait_for_exit(
        &launcher,
        &fixture.watch,
        Arc::clone(&fixture.probe),
        child.pid,
        None,
        CADENCE,
    )
    .await
    .expect("a real child reports an exit");
    assert_eq!(outcome, ExitOutcome::Code(0));
}

#[tokio::test]
async fn an_adopted_process_that_already_left_is_reported_at_once() {
    let fixture = fixture();
    let launcher = TokioProcessLauncher::default();
    launcher.adopt(ADOPTED_PID).await;
    let outcome = wait_for_exit(
        &launcher,
        &fixture.watch,
        Arc::clone(&fixture.probe),
        ADOPTED_PID,
        None,
        CADENCE,
    )
    .await
    .expect("an adopted process reports an exit");
    assert_eq!(outcome, ExitOutcome::Unobserved);
}

#[tokio::test]
async fn a_pid_the_kernel_handed_to_someone_else_counts_as_an_exit() {
    let fixture = fixture();
    fixture.mark_alive();
    let launcher = TokioProcessLauncher::default();
    launcher.adopt(ADOPTED_PID).await;
    let outcome = wait_for_exit(
        &launcher,
        &fixture.watch,
        Arc::clone(&fixture.probe),
        ADOPTED_PID,
        Some("Mon Jan 01 00:00:00 2020".to_string()),
        CADENCE,
    )
    .await
    .expect("a recycled pid reports an exit");
    assert_eq!(outcome, ExitOutcome::Unobserved);
}

#[tokio::test]
async fn a_pid_still_holding_the_recorded_identity_keeps_being_watched() {
    let fixture = fixture_that_answers_once(&format!("echo \"{ADOPTED_PID} {FIXTURE_TOKEN}\""));
    let launcher = TokioProcessLauncher::default();
    launcher.adopt(ADOPTED_PID).await;
    let outcome = wait_for_exit(
        &launcher,
        &fixture.watch,
        Arc::clone(&fixture.probe),
        ADOPTED_PID,
        Some(FIXTURE_TOKEN.to_string()),
        CADENCE,
    )
    .await;
    assert_eq!(
        outcome.expect("the adopted process left on the second poll"),
        ExitOutcome::Unobserved
    );
}

#[tokio::test]
async fn a_probe_that_cannot_answer_keeps_the_process_under_watch() {
    let fixture = fixture_that_answers_once("exit 2");
    let launcher = TokioProcessLauncher::default();
    launcher.adopt(ADOPTED_PID).await;
    let outcome = wait_for_exit(
        &launcher,
        &fixture.watch,
        Arc::clone(&fixture.probe),
        ADOPTED_PID,
        None,
        CADENCE,
    )
    .await;
    assert_eq!(
        outcome.expect("the adopted process left on the second poll"),
        ExitOutcome::Unobserved
    );
}

#[tokio::test]
async fn an_adopted_process_stops_being_tracked_once_it_leaves() {
    let fixture = fixture();
    let launcher = TokioProcessLauncher::default();
    launcher.adopt(ADOPTED_PID).await;
    assert_eq!(launcher.tracked_pids().await, vec![ADOPTED_PID]);
    wait_for_exit(
        &launcher,
        &fixture.watch,
        Arc::clone(&fixture.probe),
        ADOPTED_PID,
        None,
        CADENCE,
    )
    .await;
    assert!(launcher.tracked_pids().await.is_empty());
}

#[tokio::test]
async fn an_adopted_process_is_polled_until_it_leaves() {
    let fixture = fixture();
    fixture.mark_alive();
    let launcher = TokioProcessLauncher::default();
    launcher.adopt(ADOPTED_PID).await;

    let observer = wait_for_exit(
        &launcher,
        &fixture.watch,
        Arc::clone(&fixture.probe),
        ADOPTED_PID,
        None,
        CADENCE,
    );
    let reaper = async {
        tokio::time::sleep(std::time::Duration::from_millis(POLL_MS * 3)).await;
        fixture.mark_gone();
    };
    let (outcome, ()) = tokio::join!(observer, reaper);
    assert_eq!(
        outcome.expect("the adopted process left"),
        ExitOutcome::Unobserved
    );
}

#[test]
fn a_watch_without_an_identity_token_accepts_any_report() {
    assert!(holds_the_same_process(42, None, "any liveliness token"));
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

#[tokio::test]
async fn the_shared_poller_stops_once_the_last_watched_process_leaves() {
    let fixture = fixture();
    let launcher = TokioProcessLauncher::default();
    launcher.adopt(ADOPTED_PID).await;
    wait_for_exit(
        &launcher,
        &fixture.watch,
        Arc::clone(&fixture.probe),
        ADOPTED_PID,
        None,
        CADENCE,
    )
    .await;

    for _attempt in 0..50 {
        if !fixture.watch.state.lock().await.polling {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(POLL_MS)).await;
    }
    panic!("the shared poller should wind down once nothing is watched");
}

#[tokio::test]
async fn a_second_waiter_for_the_same_pid_does_not_release_the_first() {
    let fixture = fixture();
    fixture.mark_alive();
    let launcher = Arc::new(TokioProcessLauncher::default());
    launcher.adopt(ADOPTED_PID).await;
    let really_gone = Arc::new(AtomicBool::new(false));
    let first = {
        let launcher = Arc::clone(&launcher);
        let watch = Arc::clone(&fixture.watch);
        let probe = Arc::clone(&fixture.probe);
        let really_gone = Arc::clone(&really_gone);
        tokio::spawn(async move {
            let outcome = wait_for_exit(
                &launcher,
                &watch,
                probe,
                ADOPTED_PID,
                Some(FIXTURE_TOKEN.to_string()),
                CADENCE,
            )
            .await;
            assert!(
                really_gone.load(Ordering::SeqCst),
                "a duplicate registration must not complete the waiter that came first"
            );
            outcome
        })
    };
    tokio::time::sleep(std::time::Duration::from_millis(POLL_MS)).await;
    let recycled = wait_for_exit(
        &launcher,
        &fixture.watch,
        Arc::clone(&fixture.probe),
        ADOPTED_PID,
        Some("Mon Jan 01 00:00:00 2020".to_string()),
        CADENCE,
    );
    let reaper = async {
        tokio::time::sleep(std::time::Duration::from_millis(POLL_MS * 4)).await;
        fixture.mark_gone();
        really_gone.store(true, Ordering::SeqCst);
    };
    let (outcome, ()) = tokio::join!(recycled, reaper);
    assert_eq!(
        outcome.expect("a recycled pid reports an exit"),
        ExitOutcome::Unobserved
    );
    let first_outcome = first.await.expect("join the first waiter");
    assert_eq!(
        first_outcome.expect("the first waiter reports an exit"),
        ExitOutcome::Unobserved
    );
}

#[tokio::test]
async fn two_adopted_processes_share_one_poller() {
    let fixture = fixture();
    fixture.mark_alive();
    let launcher = TokioProcessLauncher::default();
    launcher.adopt(ADOPTED_PID).await;
    launcher.adopt(ADOPTED_PID + 1).await;

    let first = wait_for_exit(
        &launcher,
        &fixture.watch,
        Arc::clone(&fixture.probe),
        ADOPTED_PID,
        None,
        CADENCE,
    );
    let second = wait_for_exit(
        &launcher,
        &fixture.watch,
        Arc::clone(&fixture.probe),
        ADOPTED_PID + 1,
        None,
        CADENCE,
    );
    let reaper = async {
        tokio::time::sleep(std::time::Duration::from_millis(POLL_MS * 3)).await;
        fixture.mark_gone();
    };
    let (left, right, ()) = tokio::join!(first, second, reaper);
    assert!(left.is_some() && right.is_some());
}
