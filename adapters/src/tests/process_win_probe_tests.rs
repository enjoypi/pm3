use std::process::Stdio;

use tokio::process::Command;

use super::*;
use crate::platform::SleepingTree;

const POLL_MS: u64 = 20;
const MISSING_PID: u32 = 2_147_483_647;

fn probe() -> WinProcessProbe {
    WinProcessProbe::with_timeout(5_000, POLL_MS)
}

fn row(parent: Option<u32>) -> Row {
    Row {
        parent,
        memory: 4096,
        cpu_ms: 50,
        run_s: 10,
    }
}

async fn end_tree(tree: &SleepingTree) {
    let pid = tree.pid().to_string();
    Command::new("taskkill")
        .args(["/PID", &pid, "/T", "/F"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .expect("taskkill");
}

#[tokio::test]
async fn a_running_process_reports_a_stable_identity() {
    let tree = SleepingTree::spawn();
    let pid = tree.pid();
    let Liveness::Alive(first) = probe().identity(pid).await else {
        panic!("a running process is alive");
    };
    assert_eq!(probe().identity(pid).await, Liveness::Alive(first));
}

#[tokio::test]
async fn a_missing_pid_is_gone() {
    assert_eq!(probe().identity(MISSING_PID).await, Liveness::Gone);
}

#[tokio::test]
async fn identities_answer_for_every_asked_pid() {
    let tree = SleepingTree::spawn();
    let pid = tree.pid();
    let seen = probe().identities(&[pid, MISSING_PID]).await;
    assert!(
        matches!(seen.get(&pid), Some(Liveness::Alive(_))),
        "got: {seen:?}"
    );
    assert_eq!(seen.get(&MISSING_PID), Some(&Liveness::Gone));
}

#[tokio::test]
async fn a_running_tree_reports_its_resource_usage() {
    let tree = SleepingTree::spawn();
    let pid = tree.pid();
    let usage = probe().resource_usage(&[pid, MISSING_PID]).await;
    let sample = usage.get(&pid).expect("a running tree has a sample");
    assert!(sample.rss_kib > 0, "got: {sample:?}");
    assert!(!usage.contains_key(&MISSING_PID), "got: {usage:?}");
}

#[tokio::test]
async fn resident_memory_carries_the_tree_total() {
    let tree = SleepingTree::spawn();
    let pid = tree.pid();
    let memory = probe().resident_memory(&[pid]).await;
    assert!(
        memory.get(&pid).is_some_and(|kib| *kib > 0),
        "got: {memory:?}"
    );
}

#[tokio::test]
async fn waiting_for_a_missing_pid_returns_at_once() {
    assert_eq!(probe().wait_gone(MISSING_PID, 5_000).await, Liveness::Gone);
}

#[tokio::test]
async fn waiting_for_a_live_pid_gives_up_after_its_budget() {
    let tree = SleepingTree::spawn();
    let pid = tree.pid();
    let seen = probe().wait_gone(pid, POLL_MS * 3).await;
    assert!(matches!(seen, Liveness::Alive(_)), "got: {seen:?}");
}

#[tokio::test]
async fn a_tree_with_no_members_is_gone() {
    assert!(probe().wait_group_gone(MISSING_PID, 5_000).await);
}

#[tokio::test]
async fn a_live_tree_outlasts_a_short_wait() {
    let tree = SleepingTree::spawn();
    let pid = tree.pid();
    assert!(!probe().wait_group_gone(pid, POLL_MS * 3).await);
    end_tree(&tree).await;
    assert!(probe().wait_group_gone(pid, 5_000).await);
}

#[test]
fn an_orphan_still_belongs_to_the_tree_its_dead_root_started() {
    let rows = HashMap::from([(7, row(Some(3)))]);
    assert!(descends_from(&rows, 7, 3));
}

#[test]
fn a_process_without_a_parent_belongs_to_no_other_tree() {
    let rows = HashMap::from([(7, row(None))]);
    assert!(!descends_from(&rows, 7, 3));
}

#[test]
fn a_parent_cycle_left_by_pid_reuse_ends_the_walk() {
    let rows = HashMap::from([(7, row(Some(8))), (8, row(Some(7)))]);
    assert!(!descends_from(&rows, 7, 3));
}

#[test]
fn a_tree_sample_sums_every_member() {
    let rows = HashMap::from([(3, row(None)), (7, row(Some(3))), (9, row(None))]);
    let samples = tree_samples(&rows, &[3, 4]);
    assert_eq!(
        samples,
        BTreeMap::from([(
            3,
            ResourceSample {
                rss_kib: 8,
                cpu_tenths: 10,
            }
        )])
    );
}

#[test]
fn a_process_that_just_started_does_not_divide_by_zero() {
    let fresh = Row {
        run_s: 0,
        ..row(None)
    };
    assert_eq!(cpu_tenths_of(&fresh), 50);
}
