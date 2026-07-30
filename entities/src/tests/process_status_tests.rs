use super::*;

const ALL_STATUSES: [ProcessStatus; 5] = [
    ProcessStatus::Launching,
    ProcessStatus::Online,
    ProcessStatus::Stopping,
    ProcessStatus::Stopped,
    ProcessStatus::Errored,
];

#[test]
fn parse_round_trips_every_status() {
    for status in ALL_STATUSES {
        assert_eq!(ProcessStatus::parse(status.as_str()), Some(status));
    }
}

#[test]
fn parse_rejects_unknown_status() {
    assert_eq!(ProcessStatus::parse("zombie"), None);
}

#[test]
fn launching_counts_as_running() {
    assert!(ProcessStatus::Launching.is_running());
}

#[test]
fn online_counts_as_running() {
    assert!(ProcessStatus::Online.is_running());
}

#[test]
fn stopping_does_not_count_as_running() {
    assert!(!ProcessStatus::Stopping.is_running());
}

#[test]
fn stopped_does_not_count_as_running() {
    assert!(!ProcessStatus::Stopped.is_running());
}

#[test]
fn errored_does_not_count_as_running() {
    assert!(!ProcessStatus::Errored.is_running());
}

#[test]
fn stopping_is_a_shutdown_request() {
    assert!(ProcessStatus::Stopping.is_shutting_down());
}

#[test]
fn online_is_not_a_shutdown_request() {
    assert!(!ProcessStatus::Online.is_shutting_down());
}

#[test]
fn stopped_is_settled() {
    assert!(ProcessStatus::Stopped.is_settled());
}

#[test]
fn errored_is_settled() {
    assert!(ProcessStatus::Errored.is_settled());
}

#[test]
fn launching_is_not_settled() {
    assert!(!ProcessStatus::Launching.is_settled());
}

#[test]
fn online_is_not_settled() {
    assert!(!ProcessStatus::Online.is_settled());
}

#[test]
fn stopping_is_not_settled_because_pm3_still_owns_the_process() {
    assert!(!ProcessStatus::Stopping.is_settled());
}
