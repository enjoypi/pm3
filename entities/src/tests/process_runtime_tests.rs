use super::*;

fn online_at(started_at_ms: u64) -> ProcessRuntime {
    ProcessRuntime {
        status: ProcessStatus::Online,
        pid: Some(4242),
        started_at_ms: Some(started_at_ms),
        ..ProcessRuntime::new(1, "api".to_string(), started_at_ms)
    }
}

fn identity() -> ProcessIdentity {
    ProcessIdentity {
        token: "Tue Jul 28 14:06:28 2026".to_string(),
        launch_digest: "aaaa".to_string(),
        binary_digest: "bbbb".to_string(),
    }
}

#[test]
fn new_runtime_starts_stopped_without_pid() {
    let runtime = ProcessRuntime::new(7, "api".to_string(), 1000);
    assert_eq!(runtime.status, ProcessStatus::Stopped);
    assert_eq!(runtime.pid, None);
    assert_eq!(runtime.restart_time, 0);
    assert_eq!(runtime.unstable_restarts, 0);
    assert_eq!(runtime.started_at_ms, None);
}

#[test]
fn a_new_runtime_carries_no_identity() {
    let runtime = ProcessRuntime::new(7, "api".to_string(), 1000);
    assert_eq!(runtime.identity, None);
}

#[test]
fn record_identity_stores_what_the_running_process_was_launched_from() {
    let mut runtime = online_at(1000);
    runtime.record_identity(Some(identity()));
    assert_eq!(runtime.identity, Some(identity()));
}

#[test]
fn record_identity_can_clear_an_unusable_capture() {
    let mut runtime = online_at(1000);
    runtime.record_identity(Some(identity()));
    runtime.record_identity(None);
    assert_eq!(runtime.identity, None);
}

#[test]
fn mark_exited_drops_the_identity_along_with_the_pid() {
    let mut runtime = online_at(1000);
    runtime.record_identity(Some(identity()));
    runtime.mark_exited(ProcessStatus::Stopped);
    assert_eq!(runtime.identity, None);
}

#[test]
fn mark_launched_drops_a_stale_identity_until_the_new_one_is_captured() {
    let mut runtime = online_at(1000);
    runtime.record_identity(Some(identity()));
    runtime.mark_launched(99, 2000);
    assert_eq!(runtime.identity, None);
}

#[test]
fn uptime_measures_from_current_start() {
    let runtime = online_at(1000);
    assert_eq!(runtime.uptime_ms(3500), Some(2500));
}

#[test]
fn uptime_is_absent_before_first_start() {
    let runtime = ProcessRuntime::new(1, "api".to_string(), 1000);
    assert_eq!(runtime.uptime_ms(3500), None);
}

#[test]
fn uptime_is_absent_once_the_process_stopped() {
    let runtime = ProcessRuntime {
        status: ProcessStatus::Stopped,
        ..online_at(1000)
    };
    assert_eq!(runtime.uptime_ms(3500), None);
}

#[test]
fn uptime_saturates_when_clock_moves_backwards() {
    let runtime = online_at(5000);
    assert_eq!(runtime.uptime_ms(1000), Some(0));
}

#[test]
fn mark_launched_records_pid_and_start_time() {
    let mut runtime = ProcessRuntime::new(1, "api".to_string(), 1000);
    runtime.mark_launched(99, 2000);
    assert_eq!(runtime.status, ProcessStatus::Launching);
    assert_eq!(runtime.pid, Some(99));
    assert_eq!(runtime.started_at_ms, Some(2000));
}

#[test]
fn mark_online_promotes_a_launching_process() {
    let mut runtime = ProcessRuntime::new(1, "api".to_string(), 1000);
    runtime.mark_launched(99, 2000);
    runtime.mark_online();
    assert_eq!(runtime.status, ProcessStatus::Online);
}

#[test]
fn mark_stopping_keeps_the_pid_for_signalling() {
    let mut runtime = online_at(1000);
    runtime.mark_stopping();
    assert_eq!(runtime.status, ProcessStatus::Stopping);
    assert_eq!(runtime.pid, Some(4242));
}

#[test]
fn mark_exited_clears_pid_and_applies_status() {
    let mut runtime = online_at(1000);
    runtime.mark_exited(ProcessStatus::Errored);
    assert_eq!(runtime.status, ProcessStatus::Errored);
    assert_eq!(runtime.pid, None);
    assert_eq!(runtime.started_at_ms, None);
}

#[test]
fn count_restart_increments_total_and_stores_unstable_counter() {
    let mut runtime = online_at(1000);
    runtime.count_restart(3);
    assert_eq!(runtime.restart_time, 1);
    assert_eq!(runtime.unstable_restarts, 3);
}

#[test]
fn a_new_runtime_has_no_pending_restart() {
    let runtime = ProcessRuntime::new(1, "api".to_string(), 1000);
    assert!(!runtime.pending_restart);
}

#[test]
fn request_restart_records_the_intent() {
    let mut runtime = online_at(1000);
    runtime.request_restart();
    assert!(runtime.pending_restart);
}

#[test]
fn take_restart_request_consumes_the_intent() {
    let mut runtime = online_at(1000);
    runtime.request_restart();
    assert!(runtime.take_restart_request());
    assert!(!runtime.pending_restart);
}

#[test]
fn take_restart_request_is_false_without_an_intent() {
    let mut runtime = online_at(1000);
    assert!(!runtime.take_restart_request());
}

#[test]
fn count_restart_saturates_the_total() {
    let mut runtime = ProcessRuntime {
        restart_time: u32::MAX,
        ..online_at(1000)
    };
    runtime.count_restart(0);
    assert_eq!(runtime.restart_time, u32::MAX);
}
