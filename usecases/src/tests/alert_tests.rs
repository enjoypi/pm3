use super::*;

const COOLDOWN_MS: u64 = 1000;

fn notice(name: &str, kind: AlertKind) -> SupervisionEffect {
    SupervisionEffect::Notify(Alert {
        name: name.to_string(),
        kind,
    })
}

#[test]
fn the_first_alert_of_a_kind_is_raised() {
    let mut latch = AlertLatch::new(COOLDOWN_MS);

    let raised = latch.fire("api", AlertKind::LivenessKilled, 0);

    assert_eq!(raised, Some(notice("api", AlertKind::LivenessKilled)));
}

#[test]
fn a_repeat_within_the_cooldown_is_swallowed() {
    let mut latch = AlertLatch::new(COOLDOWN_MS);
    latch.fire("api", AlertKind::LivenessKilled, 0);

    let repeated = latch.fire("api", AlertKind::LivenessKilled, COOLDOWN_MS - 1);

    assert_eq!(
        repeated, None,
        "a looping failure must not flood the desktop"
    );
}

#[test]
fn a_repeat_after_the_cooldown_is_raised_again() {
    let mut latch = AlertLatch::new(COOLDOWN_MS);
    latch.fire("api", AlertKind::LivenessKilled, 0);

    let repeated = latch.fire("api", AlertKind::LivenessKilled, COOLDOWN_MS);

    assert_eq!(repeated, Some(notice("api", AlertKind::LivenessKilled)));
}

#[test]
fn another_kind_or_service_is_not_held_back() {
    let mut latch = AlertLatch::new(COOLDOWN_MS);
    latch.fire("api", AlertKind::LivenessKilled, 0);

    let memory = latch.fire("api", AlertKind::MemoryKilled, 1);
    let other = latch.fire("web", AlertKind::LivenessKilled, 1);

    assert_eq!(memory, Some(notice("api", AlertKind::MemoryKilled)));
    assert_eq!(other, Some(notice("web", AlertKind::LivenessKilled)));
}

#[test]
fn an_alarming_exit_is_raised_with_its_outcome() {
    let mut latch = AlertLatch::new(COOLDOWN_MS);

    let raised = latch.settle("api", ExitOutcome::Code(1), true, 0);

    assert_eq!(
        raised,
        Some(notice("api", AlertKind::Exited(ExitOutcome::Code(1))))
    );
}

#[test]
fn a_clean_exit_rearms_the_latch() {
    let mut latch = AlertLatch::new(COOLDOWN_MS);
    latch.settle("job", ExitOutcome::Code(1), true, 0);

    let clean = latch.settle("job", ExitOutcome::Code(0), false, 1);
    let failed_again = latch.settle("job", ExitOutcome::Code(1), true, 2);

    assert_eq!(clean, None, "a clean exit is never an alert");
    assert_eq!(
        failed_again,
        Some(notice("job", AlertKind::Exited(ExitOutcome::Code(1)))),
        "a failure after a success is news even inside the cooldown"
    );
}

#[test]
fn forgetting_one_service_keeps_the_others_latched() {
    let mut latch = AlertLatch::new(COOLDOWN_MS);
    latch.fire("api", AlertKind::MemoryKilled, 0);
    latch.fire("web", AlertKind::MemoryKilled, 0);

    latch.forget("api");

    assert_eq!(
        latch.fire("api", AlertKind::MemoryKilled, 1),
        Some(notice("api", AlertKind::MemoryKilled))
    );
    assert_eq!(latch.fire("web", AlertKind::MemoryKilled, 1), None);
}
