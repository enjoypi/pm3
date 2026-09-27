#![cfg(windows)]
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use usecases::{LaunchSpec, ProcessLauncher as _};

use super::{pure_tests::POLL_MS, *};
use crate::platform::{SHELL, SleepingTree, shell_args};

const PROBE_TIMEOUT_MS: u64 = 5000;
const MISSING_PID: u32 = 2_147_483_647;
const RECYCLED_TOKEN: &str = "0";
const CADENCE: PollCadence = PollCadence {
    interval_ms: POLL_MS,
    max_interval_ms: POLL_MS,
};

fn probe() -> Arc<HostProcessProbe> {
    Arc::new(HostProcessProbe::with_timeout(PROBE_TIMEOUT_MS, POLL_MS))
}

fn exiting_spec(dir: &tempfile::TempDir) -> LaunchSpec {
    LaunchSpec {
        name: "api".to_string(),
        program: SHELL.to_string(),
        args: shell_args("", "exit 0"),
        cwd: dir.path().to_string_lossy().into_owned(),
        env: Vec::new(),
        stdout_path: dir.path().join("out.log").to_string_lossy().into_owned(),
        stderr_path: dir.path().join("err.log").to_string_lossy().into_owned(),
    }
}

async fn adopted_exit(pid: u32, token: Option<String>) -> Option<ExitOutcome> {
    let launcher = TokioProcessLauncher::default();
    launcher.adopt(pid).await;
    let watch = Arc::new(AdoptedWatch::default());
    let outcome = wait_for_exit(&launcher, &watch, probe(), pid, token, CADENCE).await;
    assert_eq!(launcher.tracked_pids().await, Vec::<u32>::new());
    outcome
}

async fn token_of(pid: u32) -> String {
    let Liveness::Alive(token) = probe().identity(pid).await else {
        panic!("a sleeping tree is alive");
    };
    token
}

#[tokio::test]
async fn a_real_child_is_reaped_through_its_own_handle() {
    let dir = tempfile::tempdir().expect("temp dir");
    let launcher = TokioProcessLauncher::default();
    let child = launcher
        .spawn(&exiting_spec(&dir))
        .await
        .expect("should spawn the shell");
    let watch = Arc::new(AdoptedWatch::default());
    let outcome = wait_for_exit(&launcher, &watch, probe(), child.pid, None, CADENCE).await;
    assert_eq!(outcome, Some(ExitOutcome::Code(0)));
}

#[tokio::test]
async fn an_adopted_process_that_already_left_is_reported_at_once() {
    assert_eq!(
        adopted_exit(MISSING_PID, None).await,
        Some(ExitOutcome::Unobserved)
    );
}

#[tokio::test]
async fn a_pid_the_kernel_handed_to_someone_else_counts_as_an_exit() {
    let tree = SleepingTree::spawn();
    assert_eq!(
        adopted_exit(tree.pid(), Some(RECYCLED_TOKEN.to_string())).await,
        Some(ExitOutcome::Unobserved)
    );
}

#[tokio::test]
async fn a_pid_still_holding_the_recorded_identity_is_watched_until_it_leaves() {
    let tree = SleepingTree::spawn();
    let pid = tree.pid();
    let token = token_of(pid).await;
    let ended = AtomicBool::new(false);
    let observer = async {
        let outcome = adopted_exit(pid, Some(token)).await;
        assert!(
            ended.load(Ordering::SeqCst),
            "a process holding its identity must stay watched"
        );
        outcome
    };
    let reaper = async {
        tokio::time::sleep(Duration::from_millis(POLL_MS * 10)).await;
        ended.store(true, Ordering::SeqCst);
        drop(tree);
    };
    let (outcome, ()) = tokio::join!(observer, reaper);
    assert_eq!(outcome, Some(ExitOutcome::Unobserved));
}
