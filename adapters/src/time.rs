use std::time::Instant;

#[must_use]
#[expect(
    clippy::cast_possible_truncation,
    reason = "u128 毫秒截断到 u64 需 3 亿年才溢出，无安全 helper 可替代"
)]
pub fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis() as u64
}
