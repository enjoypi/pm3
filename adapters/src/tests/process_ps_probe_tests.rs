use std::{fs, os::unix::fs::PermissionsExt as _, path::PathBuf};

use super::*;

const UNUSABLE_PID: u32 = u32::MAX;

fn fake_ps(dir: &tempfile::TempDir, body: &str) -> PathBuf {
    let path = dir.path().join("ps");
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("should write a fake ps");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
        .expect("should make the fake ps executable");
    path
}

fn probe_with(body: &str) -> (tempfile::TempDir, PsProcessProbe) {
    let dir = tempfile::tempdir().expect("temp dir");
    let program = fake_ps(&dir, body).to_string_lossy().into_owned();
    (dir, PsProcessProbe::with_program(program))
}

#[tokio::test]
async fn a_live_process_reports_its_start_time_as_the_identity() {
    let token = PsProcessProbe::default()
        .identity(std::process::id())
        .await
        .into_token()
        .expect("the test process is alive");
    assert!(token.contains("20"), "got: {token}");
}

#[tokio::test]
async fn the_identity_of_a_live_process_stays_stable_across_probes() {
    let probe = PsProcessProbe::default();
    let pid = std::process::id();
    let first = probe.identity(pid).await;
    let second = probe.identity(pid).await;
    assert_eq!(first, second);
}

#[tokio::test]
async fn a_pid_no_process_can_hold_reads_as_gone() {
    assert_eq!(
        PsProcessProbe::default().identity(UNUSABLE_PID).await,
        Liveness::Gone
    );
}

#[tokio::test]
async fn a_missing_ps_program_reads_as_unreadable() {
    let probe = PsProcessProbe::with_program("/nonexistent/pm3-ps".to_string());
    assert_eq!(probe.identity(1).await, Liveness::Unreadable);
}

#[tokio::test]
async fn a_ps_that_prints_nothing_reads_as_gone() {
    let (_dir, probe) = probe_with("exit 0");
    assert_eq!(probe.identity(1).await, Liveness::Gone);
}

#[tokio::test]
async fn a_ps_that_prints_only_whitespace_reads_as_gone() {
    let (_dir, probe) = probe_with("echo '   '");
    assert_eq!(probe.identity(1).await, Liveness::Gone);
}

#[tokio::test]
async fn the_exit_code_ps_uses_for_an_unmatched_pid_reads_as_gone() {
    let (_dir, probe) = probe_with("echo 'Tue Jul 28 14:06:28 2026'; exit 1");
    assert_eq!(probe.identity(1).await, Liveness::Gone);
}

#[tokio::test]
async fn any_other_ps_failure_reads_as_unreadable() {
    let (_dir, probe) = probe_with("exit 2");
    assert_eq!(probe.identity(1).await, Liveness::Unreadable);
}

#[tokio::test]
async fn a_ps_killed_by_a_signal_reads_as_unreadable() {
    let (_dir, probe) = probe_with("kill -TERM $$");
    assert_eq!(probe.identity(1).await, Liveness::Unreadable);
}

#[tokio::test]
async fn the_identity_drops_the_padding_ps_adds() {
    let (_dir, probe) = probe_with("echo 'Tue Jul 28 14:06:28 2026    '");
    assert_eq!(
        probe.identity(1).await,
        Liveness::Alive("Tue Jul 28 14:06:28 2026".to_string())
    );
}

#[tokio::test]
async fn a_ps_that_never_answers_reads_as_unreadable() {
    let dir = tempfile::tempdir().expect("temp dir");
    let program = fake_ps(&dir, "sleep 5").to_string_lossy().into_owned();
    let probe = PsProcessProbe::new(program, 20);
    assert_eq!(probe.identity(1).await, Liveness::Unreadable);
}

#[tokio::test]
async fn the_probe_asks_ps_for_the_start_time_of_one_pid_under_a_fixed_locale() {
    let (_dir, probe) = probe_with("echo \"$LC_ALL|$*\"");
    assert_eq!(
        probe.identity(4242).await,
        Liveness::Alive("C|-ww -o lstart= -p 4242".to_string())
    );
}

#[test]
fn a_gone_process_carries_no_token() {
    assert_eq!(Liveness::Gone.into_token(), None);
}

#[test]
fn an_unreadable_probe_carries_no_token() {
    assert_eq!(Liveness::Unreadable.into_token(), None);
}
