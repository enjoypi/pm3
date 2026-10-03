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
fn each_status_answers_every_predicate() {
    let expected = [
        (ProcessStatus::Launching, true, false, false),
        (ProcessStatus::Online, true, false, false),
        (ProcessStatus::Stopping, false, true, false),
        (ProcessStatus::Stopped, false, false, true),
        (ProcessStatus::Errored, false, false, true),
    ];
    for (status, running, shutting_down, settled) in expected {
        assert_eq!(status.is_running(), running, "{status:?} is_running");
        assert_eq!(
            status.is_shutting_down(),
            shutting_down,
            "{status:?} is_shutting_down"
        );
        assert_eq!(status.is_settled(), settled, "{status:?} is_settled");
    }
}
