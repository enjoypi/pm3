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
async fn a_pid_the_system_rejects_has_no_identity() {
    assert_eq!(PsProcessProbe::default().identity(UNUSABLE_PID).await, None);
}

#[tokio::test]
async fn a_missing_ps_program_yields_no_identity() {
    let probe = PsProcessProbe::with_program("/nonexistent/pm3-ps".to_string());
    assert_eq!(probe.identity(1).await, None);
}

#[tokio::test]
async fn a_ps_that_prints_nothing_yields_no_identity() {
    let (_dir, probe) = probe_with("exit 0");
    assert_eq!(probe.identity(1).await, None);
}

#[tokio::test]
async fn a_ps_that_prints_only_whitespace_yields_no_identity() {
    let (_dir, probe) = probe_with("echo '   '");
    assert_eq!(probe.identity(1).await, None);
}

#[tokio::test]
async fn a_failing_ps_yields_no_identity_even_when_it_printed_something() {
    let (_dir, probe) = probe_with("echo 'Tue Jul 28 14:06:28 2026'; exit 1");
    assert_eq!(probe.identity(1).await, None);
}

#[tokio::test]
async fn the_identity_drops_the_padding_ps_adds() {
    let (_dir, probe) = probe_with("echo 'Tue Jul 28 14:06:28 2026    '");
    assert_eq!(
        probe.identity(1).await,
        Some("Tue Jul 28 14:06:28 2026".to_string())
    );
}

#[tokio::test]
async fn the_probe_asks_ps_for_the_start_time_of_one_pid_under_a_fixed_locale() {
    let (_dir, probe) = probe_with("echo \"$LC_ALL|$*\"");
    assert_eq!(
        probe.identity(4242).await,
        Some("C|-ww -o lstart= -p 4242".to_string())
    );
}
