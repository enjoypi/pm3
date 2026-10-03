#![cfg(unix)]
use std::{
    fs,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use usecases::{LaunchSpec, ProcessLauncher as _};

use super::{
    pure_tests::{ADOPTED_PID, FIXTURE_TOKEN, POLL_MS},
    *,
};

const POLL_STEP_MS: u64 = 20;

const PROBE_TIMEOUT_MS: u64 = 5000;
const CADENCE: PollCadence = PollCadence {
    interval_ms: POLL_MS,
    max_interval_ms: POLL_MS,
};

struct Fixture {
    dir: tempfile::TempDir,
    probe: Arc<HostProcessProbe>,
    watch: Arc<AdoptedWatch>,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().expect("temp dir");
    let alive = dir.path().join("alive");
    let body = format!(
        concat!(
            "if [ ! -f {} ]; then exit 1; fi\n",
            "for pid in $(echo \"$5\" | tr ',' ' '); do echo \"$pid {}\"; done",
        ),
        alive.display(),
        FIXTURE_TOKEN,
    );
    let script = crate::platform::script(dir.path(), "ps", &body, "");
    let probe = Arc::new(HostProcessProbe::new(
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
    let body = format!(
        concat!(
            "if [ -f \"$0.asked\" ]; then exit 1; fi\n",
            "touch \"$0.asked\"\n",
            "{}",
        ),
        first_answer,
    );
    let script = crate::platform::script(dir.path(), "ps", &body, "");
    Fixture {
        dir,
        probe: Arc::new(HostProcessProbe::new(
            script.to_string_lossy().into_owned(),
            PROBE_TIMEOUT_MS,
            POLL_STEP_MS,
        )),
        watch: Arc::new(AdoptedWatch::default()),
    }
}

impl Fixture {
    async fn watched(
        &self,
        launcher: &TokioProcessLauncher,
        pid: u32,
        token: Option<String>,
    ) -> Option<ExitOutcome> {
        wait_for_exit(
            launcher,
            &self.watch,
            Arc::clone(&self.probe),
            pid,
            token,
            CADENCE,
        )
        .await
    }

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
    let outcome = fixture
        .watched(&launcher, child.pid, None)
        .await
        .expect("a real child reports an exit");
    assert_eq!(outcome, ExitOutcome::Code(0));
}

#[tokio::test]
async fn an_adopted_process_that_already_left_is_reported_at_once() {
    let fixture = fixture();
    let launcher = TokioProcessLauncher::default();
    launcher.adopt(ADOPTED_PID).await;
    let outcome = fixture
        .watched(&launcher, ADOPTED_PID, None)
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
    let outcome = fixture
        .watched(
            &launcher,
            ADOPTED_PID,
            Some("Mon Jan 01 00:00:00 2020".to_string()),
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
    let outcome = fixture
        .watched(&launcher, ADOPTED_PID, Some(FIXTURE_TOKEN.to_string()))
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
    let outcome = fixture.watched(&launcher, ADOPTED_PID, None).await;
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
    fixture.watched(&launcher, ADOPTED_PID, None).await;
    assert_eq!(launcher.tracked_pids().await, Vec::<u32>::new());
}

#[tokio::test]
async fn an_adopted_process_is_polled_until_it_leaves() {
    let fixture = fixture();
    fixture.mark_alive();
    let launcher = TokioProcessLauncher::default();
    launcher.adopt(ADOPTED_PID).await;

    let observer = fixture.watched(&launcher, ADOPTED_PID, None);
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

#[tokio::test]
async fn the_shared_poller_stops_once_the_last_watched_process_leaves() {
    let fixture = fixture();
    let launcher = TokioProcessLauncher::default();
    launcher.adopt(ADOPTED_PID).await;
    fixture.watched(&launcher, ADOPTED_PID, None).await;

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
    let recycled = fixture.watched(
        &launcher,
        ADOPTED_PID,
        Some("Mon Jan 01 00:00:00 2020".to_string()),
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

    let first = fixture.watched(&launcher, ADOPTED_PID, None);
    let second = fixture.watched(&launcher, ADOPTED_PID + 1, None);
    let reaper = async {
        tokio::time::sleep(std::time::Duration::from_millis(POLL_MS * 3)).await;
        fixture.mark_gone();
    };
    let (left, right, ()) = tokio::join!(first, second, reaper);
    assert!(left.is_some() && right.is_some());
}
