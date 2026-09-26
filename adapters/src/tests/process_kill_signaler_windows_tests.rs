#![cfg(windows)]
use usecases::SignalScope;

use super::*;
use crate::{config::STOP_SIGNAL_TERM, platform::SleepingTree};

const TIMEOUT_MS: u64 = 10_000;
const TASKKILL: &str = "taskkill";
const MISSING_PID: u32 = 2_147_483_647;

fn signaler() -> KillSignaler {
    KillSignaler::with_stop_signal(STOP_SIGNAL_TERM.to_string(), TIMEOUT_MS, TASKKILL)
}

#[tokio::test]
async fn terminating_ends_the_whole_tree() {
    let mut tree = SleepingTree::spawn();
    signaler()
        .terminate(tree.pid(), SignalScope::ProcessGroup)
        .await
        .expect("taskkill should end the tree");
    let status = tree.0.wait().await.expect("reap");
    assert!(!status.success(), "got: {status}");
}

#[tokio::test]
async fn force_killing_ends_the_whole_tree() {
    let mut tree = SleepingTree::spawn();
    signaler()
        .force_kill(tree.pid(), SignalScope::SinglePid)
        .await
        .expect("taskkill should end the tree");
    let status = tree.0.wait().await.expect("reap");
    assert!(!status.success(), "got: {status}");
}

#[tokio::test]
async fn the_configured_stop_signal_is_delivered_by_name() {
    let mut tree = SleepingTree::spawn();
    signaler()
        .deliver(STOP_SIGNAL_TERM, tree.pid(), SignalScope::SinglePid)
        .await
        .expect("the stop signal ends the tree");
    tree.0.wait().await.expect("reap");
}

#[tokio::test]
async fn a_signal_windows_cannot_express_is_refused_instead_of_killing() {
    let mut tree = SleepingTree::spawn();
    let pid = tree.pid();
    let err = signaler()
        .deliver("USR1", pid, SignalScope::SinglePid)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("windows has no SIGUSR1"), "got: {err}");
    assert!(
        tree.0.try_wait().expect("poll the child").is_none(),
        "a refused signal must leave the process running"
    );
}

#[tokio::test]
async fn a_pid_outside_the_safe_range_is_refused() {
    for pid in [0, 1, u32::MAX] {
        let err = signaler()
            .terminate(pid, SignalScope::SinglePid)
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains(UNSAFE_PID_REASON), "pid {pid} got: {err}");
    }
}

#[tokio::test]
async fn a_missing_taskkill_is_reported() {
    let signaler = KillSignaler::with_stop_signal(
        STOP_SIGNAL_TERM.to_string(),
        TIMEOUT_MS,
        r"C:\nonexistent\taskkill.exe",
    );
    let err = signaler
        .terminate(MISSING_PID, SignalScope::SinglePid)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.starts_with("cannot signal pid"), "got: {err}");
}

#[tokio::test]
async fn taskkill_refusing_an_unknown_pid_is_reported() {
    let err = signaler()
        .terminate(MISSING_PID, SignalScope::SinglePid)
        .await
        .unwrap_err()
        .to_string();
    assert!(
        err.starts_with(&format!("cannot signal pid {MISSING_PID}")),
        "got: {err}"
    );
}

#[tokio::test]
async fn a_taskkill_that_never_answers_is_given_up_on() {
    let dir = tempfile::tempdir().expect("temp dir");
    let stalling = dir.path().join("taskkill.cmd");
    std::fs::write(
        &stalling,
        "@C:\\Windows\\System32\\PING.EXE -n 3 127.0.0.1 >NUL\r\n",
    )
    .expect("write the stalling stub");
    let signaler = KillSignaler::with_stop_signal(
        STOP_SIGNAL_TERM.to_string(),
        50,
        &stalling.to_string_lossy(),
    );
    let err = signaler
        .terminate(MISSING_PID, SignalScope::SinglePid)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("did not answer within 50ms"), "got: {err}");
}
