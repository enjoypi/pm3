use std::{collections::HashMap, fs};

use tokio::sync::oneshot;

use super::*;

pub const POLL_MS: u64 = 10;
pub const ADOPTED_PID: u32 = 4242;
pub const FIXTURE_TOKEN: &str = "Tue Jul 28 14:06:28 2026";

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

#[test]
fn a_watch_holding_the_recorded_identity_keeps_watching() {
    assert!(holds_the_same_process(
        42,
        Some(FIXTURE_TOKEN),
        FIXTURE_TOKEN
    ));
}

#[test]
fn a_watch_seeing_another_identity_lets_the_pid_go() {
    assert!(!holds_the_same_process(
        42,
        Some(FIXTURE_TOKEN),
        "Mon Jan 01 00:00:00 2020"
    ));
}

#[test]
fn a_probe_that_cannot_read_the_pid_keeps_it_under_watch() {
    assert!(still_running(42, None, Some(&Liveness::Unreadable)));
}

#[test]
fn a_pid_missing_from_the_snapshot_keeps_it_under_watch() {
    assert!(still_running(42, None, None));
}

#[test]
fn a_gone_pid_is_no_longer_running() {
    assert!(!still_running(42, None, Some(&Liveness::Gone)));
}

#[test]
fn an_alive_pid_is_still_running() {
    assert!(still_running(
        42,
        None,
        Some(&Liveness::Alive(FIXTURE_TOKEN.to_string()))
    ));
}
