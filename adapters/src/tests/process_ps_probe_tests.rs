#![cfg(unix)]
use std::{fs, os::unix::fs::PermissionsExt as _, path::PathBuf};

use super::*;

const POLL_STEP_MS: u64 = 20;

const UNUSABLE_PID: u32 = u32::MAX;
const PROBE_TIMEOUT_MS: u64 = 5000;

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
    (
        dir,
        PsProcessProbe::new(program, PROBE_TIMEOUT_MS, POLL_STEP_MS),
    )
}

#[tokio::test]
async fn a_live_process_reports_its_start_time_as_the_identity() {
    let token = PsProcessProbe::with_timeout(PROBE_TIMEOUT_MS, POLL_STEP_MS)
        .identity(std::process::id())
        .await
        .into_token()
        .expect("the test process is alive");
    assert!(token.contains("20"), "got: {token}");
}

#[tokio::test]
async fn the_identity_of_a_live_process_stays_stable_across_probes() {
    let probe = PsProcessProbe::with_timeout(PROBE_TIMEOUT_MS, POLL_STEP_MS);
    let pid = std::process::id();
    let first = probe.identity(pid).await;
    let second = probe.identity(pid).await;
    assert_eq!(first, second);
}

#[tokio::test]
async fn a_pid_no_process_can_hold_reads_as_gone() {
    assert_eq!(
        PsProcessProbe::with_timeout(PROBE_TIMEOUT_MS, POLL_STEP_MS)
            .identity(UNUSABLE_PID)
            .await,
        Liveness::Gone
    );
}

#[tokio::test]
async fn a_missing_ps_program_reads_as_unreadable() {
    let probe = PsProcessProbe::new(
        "/nonexistent/pm3-ps".to_string(),
        PROBE_TIMEOUT_MS,
        POLL_STEP_MS,
    );
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
async fn wait_gone_returns_as_soon_as_the_process_leaves() {
    let (_dir, probe) = probe_with("exit 0");
    assert_eq!(probe.wait_gone(7, 60_000).await, Liveness::Gone);
}

#[tokio::test]
async fn wait_gone_probes_again_while_the_budget_still_has_room() {
    let (_dir, probe) = probe_with(concat!(
        "if [ -f \"$0.asked\" ]; then exit 1; fi\n",
        "touch \"$0.asked\"\n",
        "echo '7 Tue Jul 28 14:06:28 2026'",
    ));
    assert_eq!(probe.wait_gone(7, 60_000).await, Liveness::Gone);
}

#[tokio::test]
async fn wait_gone_reports_the_last_known_state_once_the_budget_runs_out() {
    let (_dir, probe) = probe_with("echo '  7 Tue Jul 28 14:06:28 2026'");
    assert_eq!(
        probe.wait_gone(7, 30).await,
        Liveness::Alive("Tue Jul 28 14:06:28 2026".to_string())
    );
}

#[tokio::test]
async fn wait_gone_stops_polling_when_the_budget_is_already_spent() {
    let (_dir, probe) = probe_with("exit 2");
    assert_eq!(probe.wait_gone(7, 0).await, Liveness::Unreadable);
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
    let (_dir, probe) = probe_with("echo '  1 Tue Jul 28 14:06:28 2026    '");
    assert_eq!(
        probe.identity(1).await,
        Liveness::Alive("Tue Jul 28 14:06:28 2026".to_string())
    );
}

#[tokio::test]
async fn a_ps_that_never_answers_reads_as_unreadable() {
    let dir = tempfile::tempdir().expect("temp dir");
    let program = fake_ps(&dir, "sleep 5").to_string_lossy().into_owned();
    let probe = PsProcessProbe::new(program, 20, POLL_STEP_MS);
    assert_eq!(probe.identity(1).await, Liveness::Unreadable);
}

#[tokio::test]
async fn the_probe_asks_ps_for_the_start_time_of_one_pid_under_a_fixed_locale() {
    let (_dir, probe) = probe_with("echo \"4242 $LC_ALL|$*\"");
    assert_eq!(
        probe.identity(4242).await,
        Liveness::Alive("C|-ww -o pid=,lstart= -p 4242".to_string())
    );
}

#[tokio::test]
async fn probing_nothing_asks_ps_nothing() {
    let (_dir, probe) = probe_with("exit 2");
    assert!(probe.identities(&[]).await.is_empty());
}

#[tokio::test]
async fn one_batch_call_covers_every_watched_pid() {
    let (_dir, probe) =
        probe_with("echo \"$*\" >> \"$0.calls\"; echo '7 Tue Jul 28 14:06:28 2026'");
    let seen = probe.identities(&[7, 8]).await;
    assert_eq!(
        seen.get(&7),
        Some(&Liveness::Alive("Tue Jul 28 14:06:28 2026".to_string()))
    );
}

#[tokio::test]
async fn a_pid_the_batch_does_not_list_reads_as_gone() {
    let (_dir, probe) = probe_with("echo '7 Tue Jul 28 14:06:28 2026'");
    let seen = probe.identities(&[7, 8]).await;
    assert_eq!(seen.get(&8), Some(&Liveness::Gone));
}

#[tokio::test]
async fn a_batch_call_that_ps_refuses_leaves_every_pid_unreadable() {
    let (_dir, probe) = probe_with("exit 2");
    let seen = probe.identities(&[7, 8]).await;
    assert_eq!(seen.get(&7), Some(&Liveness::Unreadable));
    assert_eq!(seen.get(&8), Some(&Liveness::Unreadable));
}

#[tokio::test]
async fn a_batch_call_ps_cannot_answer_leaves_every_pid_unreadable() {
    let dir = tempfile::tempdir().expect("temp dir");
    let program = fake_ps(&dir, "sleep 5").to_string_lossy().into_owned();
    let probe = PsProcessProbe::new(program, 20, POLL_STEP_MS);
    let seen = probe.identities(&[7, 8]).await;
    assert_eq!(seen.get(&8), Some(&Liveness::Unreadable));
}

#[tokio::test]
async fn a_missing_ps_leaves_every_batched_pid_unreadable() {
    let probe = PsProcessProbe::new(
        "/nonexistent/pm3-ps".to_string(),
        PROBE_TIMEOUT_MS,
        POLL_STEP_MS,
    );
    let seen = probe.identities(&[7]).await;
    assert_eq!(seen.get(&7), Some(&Liveness::Unreadable));
}

#[tokio::test]
async fn a_batch_line_without_a_numeric_pid_is_ignored() {
    let (_dir, probe) = probe_with("echo 'header Tue Jul 28 14:06:28 2026'");
    assert_eq!(probe.identities(&[7]).await.get(&7), Some(&Liveness::Gone));
}

#[test]
fn a_gone_process_carries_no_token() {
    assert_eq!(Liveness::Gone.into_token(), None);
}

#[test]
fn an_unreadable_probe_carries_no_token() {
    assert_eq!(Liveness::Unreadable.into_token(), None);
}

#[tokio::test]
async fn a_batch_line_without_a_start_time_is_ignored() {
    let (_dir, probe) = probe_with("echo '7    '");
    assert_eq!(probe.identities(&[7]).await.get(&7), Some(&Liveness::Gone));
}

#[tokio::test]
async fn a_live_process_reports_a_resident_memory_footprint() {
    let sampled = PsProcessProbe::with_timeout(PROBE_TIMEOUT_MS, POLL_STEP_MS)
        .resident_memory(&[std::process::id()])
        .await;
    let rss = sampled
        .get(&std::process::id())
        .copied()
        .expect("the test process occupies memory");
    assert!(rss > 0, "got: {rss}");
}

#[tokio::test]
async fn an_empty_batch_samples_no_memory_at_all() {
    let (_dir, probe) = probe_with("echo unreachable");
    assert!(probe.resident_memory(&[]).await.is_empty());
}

#[tokio::test]
async fn a_memory_sample_ps_refuses_reports_nothing() {
    let (_dir, probe) = probe_with("exit 2");
    assert!(probe.resident_memory(&[7]).await.is_empty());
}

#[tokio::test]
async fn a_memory_sample_ps_cannot_answer_in_time_reports_nothing() {
    let dir = tempfile::tempdir().expect("temp dir");
    let program = fake_ps(&dir, "sleep 5").to_string_lossy().into_owned();
    let probe = PsProcessProbe::new(program, 20, POLL_STEP_MS);
    assert!(probe.resident_memory(&[7]).await.is_empty());
}

#[tokio::test]
async fn a_missing_ps_samples_no_memory() {
    let probe = PsProcessProbe::new(
        "/nonexistent/ps".to_string(),
        PROBE_TIMEOUT_MS,
        POLL_STEP_MS,
    );
    assert!(probe.resident_memory(&[7]).await.is_empty());
}

#[tokio::test]
async fn an_unparsable_memory_line_is_skipped() {
    let (_dir, probe) = probe_with("echo '7 plenty'; echo '8 4096'");
    let sampled = probe.resident_memory(&[7, 8]).await;
    assert_eq!(sampled.get(&7), None);
    assert_eq!(sampled.get(&8), Some(&4096));
}

#[tokio::test]
async fn a_memory_line_without_a_numeric_pid_is_skipped() {
    let (_dir, probe) = probe_with("echo 'header rss'; echo '8 4096'");
    let sampled = probe.resident_memory(&[8]).await;
    assert_eq!(sampled.get(&8), Some(&4096));
    assert_eq!(sampled.len(), 1);
}

#[tokio::test]
async fn a_memory_sample_for_a_pid_that_already_left_reports_nothing() {
    let (_dir, probe) = probe_with("exit 1");
    assert!(probe.resident_memory(&[7]).await.is_empty());
}

#[tokio::test]
async fn a_memory_line_without_a_separator_is_skipped() {
    let (_dir, probe) = probe_with("echo 'unsplittable'; echo '8 4096'");
    let sampled = probe.resident_memory(&[8]).await;
    assert_eq!(sampled.len(), 1);
}

#[tokio::test]
async fn a_live_process_reports_its_resource_usage() {
    let sampled = PsProcessProbe::with_timeout(PROBE_TIMEOUT_MS, POLL_STEP_MS)
        .resource_usage(&[std::process::id()])
        .await;
    let sample = sampled
        .get(&std::process::id())
        .copied()
        .expect("the test process occupies memory");
    assert!(sample.rss_kib > 0, "got: {sample:?}");
}

#[tokio::test]
async fn an_empty_batch_samples_no_resources_at_all() {
    let (_dir, probe) = probe_with("echo unreachable");
    assert!(probe.resource_usage(&[]).await.is_empty());
}

#[tokio::test]
async fn a_resource_sample_ps_refuses_reports_nothing() {
    let (_dir, probe) = probe_with("exit 2");
    assert!(probe.resource_usage(&[7]).await.is_empty());
}

#[tokio::test]
async fn a_resource_report_carries_rss_and_cpu_side_by_side() {
    let (_dir, probe) = probe_with("echo '7 4096 12.3'; echo '8 2048 0.0'");
    let sampled = probe.resource_usage(&[7, 8]).await;
    assert_eq!(
        sampled.get(&7),
        Some(&ResourceSample {
            rss_kib: 4096,
            cpu_tenths: 123,
        })
    );
    assert_eq!(
        sampled.get(&8),
        Some(&ResourceSample {
            rss_kib: 2048,
            cpu_tenths: 0,
        })
    );
}

#[tokio::test]
async fn a_cpu_value_without_a_fraction_reads_as_whole_percents() {
    let (_dir, probe) = probe_with("echo '7 4096 12'");
    let sampled = probe.resource_usage(&[7]).await;
    assert_eq!(
        sampled.get(&7),
        Some(&ResourceSample {
            rss_kib: 4096,
            cpu_tenths: 120,
        })
    );
}

#[tokio::test]
async fn a_resource_line_with_a_broken_cpu_is_skipped() {
    let (_dir, probe) = probe_with("echo '7 4096 12.'; echo '8 2048 0.5'");
    let sampled = probe.resource_usage(&[7, 8]).await;
    assert_eq!(sampled.get(&7), None);
    assert_eq!(
        sampled.get(&8),
        Some(&ResourceSample {
            rss_kib: 2048,
            cpu_tenths: 5,
        })
    );
}

#[tokio::test]
async fn a_resource_line_with_a_broken_rss_is_skipped() {
    let (_dir, probe) = probe_with("echo '7 plenty 0.5'; echo '8 2048 0.5'");
    let sampled = probe.resource_usage(&[7, 8]).await;
    assert_eq!(sampled.get(&7), None);
    assert!(sampled.contains_key(&8));
}

#[tokio::test]
async fn a_resource_line_that_ends_early_is_skipped() {
    let (_dir, probe) = probe_with("echo '7 4096'; echo '8 2048 0.5'");
    let sampled = probe.resource_usage(&[7, 8]).await;
    assert_eq!(sampled.get(&7), None);
    assert!(sampled.contains_key(&8));
}

#[tokio::test]
async fn malformed_resource_lines_are_skipped_one_by_one() {
    let body = "echo ''; echo 'abc 2048 0.5'; echo '7'; echo '8 xx 0.5'; echo '9 2048 xx.5'; echo '11 2048 1.x'; echo '12 1024 0.5'";
    let (_dir, probe) = probe_with(body);
    let sampled = probe.resource_usage(&[12]).await;
    assert_eq!(sampled.len(), 1);
    assert_eq!(
        sampled.get(&12),
        Some(&ResourceSample {
            rss_kib: 1024,
            cpu_tenths: 5,
        })
    );
}
