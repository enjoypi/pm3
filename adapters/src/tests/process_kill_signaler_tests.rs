use tokio::process::{Child, Command};

use super::*;

const UNUSABLE_PID: u32 = u32::MAX;

fn spawn_sleeper() -> Child {
    Command::new("/bin/sh")
        .args(["-c", "sleep 30"])
        .kill_on_drop(true)
        .spawn()
        .expect("should spawn a sleeper")
}

fn pid_of(child: &Child) -> u32 {
    child.id().expect("a freshly spawned child reports a pid")
}

#[tokio::test]
async fn terminate_signals_a_running_child() {
    let mut child = spawn_sleeper();
    let pid = pid_of(&child);
    KillSignaler::default()
        .terminate(pid)
        .await
        .expect("should signal");
    let status = child.wait().await.expect("should reap");
    assert!(
        !status.success(),
        "a terminated child should not exit clean"
    );
}

#[tokio::test]
async fn force_kill_signals_a_running_child() {
    let mut child = spawn_sleeper();
    let pid = pid_of(&child);
    KillSignaler::default()
        .force_kill(pid)
        .await
        .expect("should signal");
    let status = child.wait().await.expect("should reap");
    assert_eq!(status.code(), None);
}

#[tokio::test]
async fn terminate_reports_a_pid_the_system_rejects() {
    let err = KillSignaler::default()
        .terminate(UNUSABLE_PID)
        .await
        .unwrap_err()
        .to_string();
    assert!(
        err.contains(&format!("cannot signal pid {UNUSABLE_PID}")),
        "got: {err}"
    );
}

#[tokio::test]
async fn terminate_explains_why_the_system_refused() {
    let err = KillSignaler::default()
        .terminate(UNUSABLE_PID)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("illegal process id"), "got: {err}");
}

#[tokio::test]
async fn force_kill_reports_a_pid_the_system_rejects() {
    let err = KillSignaler::default()
        .force_kill(UNUSABLE_PID)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot signal pid"), "got: {err}");
}

#[tokio::test]
async fn terminate_reports_a_missing_kill_program() {
    let signaler = KillSignaler::with_program("/nonexistent/pm3-kill".to_string());
    let err = signaler.terminate(1).await.unwrap_err().to_string();
    assert!(err.contains("cannot signal pid 1"), "got: {err}");
}

#[tokio::test]
async fn terminate_falls_back_to_the_exit_status_when_kill_stays_silent() {
    let signaler = KillSignaler::with_program("/usr/bin/false".to_string());
    let err = signaler.terminate(1).await.unwrap_err().to_string();
    assert!(err.contains("exited with status"), "got: {err}");
}
