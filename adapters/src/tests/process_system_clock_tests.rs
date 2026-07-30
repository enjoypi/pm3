use super::*;

const EPOCH_2023_MS: u64 = 1_700_000_000_000;

#[test]
fn now_ms_reports_a_wall_clock_reading() {
    assert!(SystemClock.now_ms() > EPOCH_2023_MS);
}

#[test]
fn now_ms_never_moves_backwards() {
    let earlier = SystemClock.now_ms();
    assert!(SystemClock.now_ms() >= earlier);
}
