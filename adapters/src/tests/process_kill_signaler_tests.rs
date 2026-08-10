#![cfg(unix)]
use std::{os::unix::fs::PermissionsExt as _, process::Stdio, time::Duration};

use tempfile::TempDir;
use tokio::{
    io::{AsyncBufReadExt as _, BufReader},
    process::{Child, Command},
};
use usecases::{Liveness, ProcessProbe as _, SignalScope};

use super::*;

const POLL_STEP_MS: u64 = 20;
use crate::{config::STOP_SIGNAL_TERM, process::ps_probe::PsProcessProbe};

const MISSING_PID: u32 = 2_147_483_647;
const BROADCAST_PID: u32 = u32::MAX;
const SELF_GROUP_PID: u32 = 0;
const INIT_PID: u32 = 1;
const DEATH_POLLS: u32 = 100;
const DEATH_POLL_INTERVAL_MS: u64 = 20;
const STALL_TIMEOUT_MS: u64 = 20;
const SIGNAL_TIMEOUT_MS: u64 = 5000;

const TASKKILL_PATH: &str = "taskkill";

fn signaler() -> KillSignaler {
    KillSignaler::with_stop_signal(
        STOP_SIGNAL_TERM.to_string(),
        SIGNAL_TIMEOUT_MS,
        TASKKILL_PATH,
    )
}

fn spawn_sleeper() -> Child {
    Command::new("/bin/sh")
        .args(["-c", "exec sleep 30"])
        .kill_on_drop(true)
        .spawn()
        .expect("should spawn a sleeper")
}

fn spawn_group_leader_with_grandchild() -> Child {
    Command::new("/bin/sh")
        .args(["-c", "sleep 30 & echo $!; wait"])
        .process_group(0)
        .stdout(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("should spawn a group leader")
}

fn pid_of(child: &Child) -> u32 {
    child.id().expect("a freshly spawned child reports a pid")
}

async fn announced_pid(child: &mut Child) -> u32 {
    let stdout = child.stdout.take().expect("the leader pipes its stdout");
    let mut line = String::new();
    BufReader::new(stdout)
        .read_line(&mut line)
        .await
        .expect("the leader announces its grandchild");
    line.trim()
        .parse()
        .expect("the announced grandchild pid is a number")
}

async fn outlives_its_group(pid: u32) -> bool {
    let probe = PsProcessProbe::with_timeout(SIGNAL_TIMEOUT_MS, POLL_STEP_MS);
    for _poll in 0..DEATH_POLLS {
        if probe.identity(pid).await == Liveness::Gone {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(DEATH_POLL_INTERVAL_MS)).await;
    }
    true
}

fn stalling_kill(dir: &TempDir) -> String {
    let path = dir.path().join("stalling-kill");
    std::fs::write(&path, "#!/bin/sh\nsleep 5\n").expect("write the stand-in");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
        .expect("make the stand-in executable");
    path.to_string_lossy().into_owned()
}

#[tokio::test]
async fn terminate_signals_a_running_child() {
    let mut child = spawn_sleeper();
    let pid = pid_of(&child);
    signaler()
        .terminate(pid, SignalScope::ProcessGroup)
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
    signaler()
        .force_kill(pid, SignalScope::ProcessGroup)
        .await
        .expect("should signal");
    let status = child.wait().await.expect("should reap");
    assert_eq!(status.code(), None);
}

#[tokio::test]
async fn terminate_reaches_a_grandchild_through_the_process_group() {
    let mut child = spawn_group_leader_with_grandchild();
    let grandchild = announced_pid(&mut child).await;
    signaler()
        .terminate(pid_of(&child), SignalScope::ProcessGroup)
        .await
        .expect("should signal");
    child.wait().await.expect("should reap");
    assert!(
        !outlives_its_group(grandchild).await,
        "a grandchild must die with the process group pm3 spawned"
    );
}

#[tokio::test]
async fn terminate_honours_the_configured_stop_signal() {
    let mut child = spawn_sleeper();
    let pid = pid_of(&child);
    KillSignaler::with_stop_signal("INT".to_string(), SIGNAL_TIMEOUT_MS, TASKKILL_PATH)
        .terminate(pid, SignalScope::ProcessGroup)
        .await
        .expect("should signal");
    let status = child.wait().await.expect("should reap");
    assert!(
        !status.success(),
        "an interrupted child should not exit clean"
    );
}

#[tokio::test]
async fn terminate_gives_up_on_a_kill_program_that_never_answers() {
    let dir = TempDir::new().expect("temp dir");
    let signaler = KillSignaler::new(stalling_kill(&dir), "TERM".to_string(), STALL_TIMEOUT_MS);
    let err = signaler
        .terminate(2, SignalScope::ProcessGroup)
        .await
        .unwrap_err()
        .to_string();
    assert!(
        err.contains(&format!("did not answer within {STALL_TIMEOUT_MS}ms")),
        "got: {err}"
    );
}

#[tokio::test]
async fn terminate_reports_a_pid_the_system_rejects() {
    let err = signaler()
        .terminate(MISSING_PID, SignalScope::ProcessGroup)
        .await
        .unwrap_err()
        .to_string();
    assert!(
        err.contains(&format!("cannot signal pid {MISSING_PID}")),
        "got: {err}"
    );
}

#[tokio::test]
async fn terminate_explains_why_the_system_refused() {
    let err = signaler()
        .terminate(MISSING_PID, SignalScope::ProcessGroup)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("No such process"), "got: {err}");
}

#[tokio::test]
async fn force_kill_reports_a_pid_the_system_rejects() {
    let err = signaler()
        .force_kill(MISSING_PID, SignalScope::ProcessGroup)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot signal pid"), "got: {err}");
}

#[tokio::test]
async fn terminate_reports_a_missing_kill_program() {
    let signaler = KillSignaler::new(
        "/nonexistent/pm3-kill".to_string(),
        STOP_SIGNAL_TERM.to_string(),
        SIGNAL_TIMEOUT_MS,
    );
    let err = signaler
        .terminate(2, SignalScope::ProcessGroup)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot signal pid 2"), "got: {err}");
}

#[tokio::test]
async fn terminate_falls_back_to_the_exit_status_when_kill_stays_silent() {
    let signaler = KillSignaler::new(
        "/usr/bin/false".to_string(),
        STOP_SIGNAL_TERM.to_string(),
        SIGNAL_TIMEOUT_MS,
    );
    let err = signaler
        .terminate(2, SignalScope::ProcessGroup)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("exited with status"), "got: {err}");
}

#[tokio::test]
async fn terminate_refuses_a_pid_that_would_widen_into_a_broadcast() {
    let err = signaler()
        .terminate(BROADCAST_PID, SignalScope::ProcessGroup)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("outside the safe range"), "got: {err}");
}

#[tokio::test]
async fn force_kill_refuses_a_pid_that_would_widen_into_a_broadcast() {
    let err = signaler()
        .force_kill(BROADCAST_PID, SignalScope::ProcessGroup)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("outside the safe range"), "got: {err}");
}

#[tokio::test]
async fn terminate_refuses_the_pid_that_means_the_calling_group() {
    let err = signaler()
        .terminate(SELF_GROUP_PID, SignalScope::ProcessGroup)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("outside the safe range"), "got: {err}");
}

#[tokio::test]
async fn terminate_refuses_the_init_pid() {
    let err = signaler()
        .terminate(INIT_PID, SignalScope::ProcessGroup)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("outside the safe range"), "got: {err}");
}

#[tokio::test]
async fn a_single_pid_scope_never_signals_the_process_group() {
    let mut child = spawn_group_leader_with_grandchild();
    let grandchild = announced_pid(&mut child).await;
    signaler()
        .terminate(pid_of(&child), SignalScope::SinglePid)
        .await
        .expect("should signal");
    child.wait().await.expect("should reap");
    let survived = outlives_its_group(grandchild).await;
    signaler()
        .force_kill(grandchild, SignalScope::SinglePid)
        .await
        .ok();
    assert!(
        survived,
        "an unverified pid must not take its neighbours down"
    );
}
