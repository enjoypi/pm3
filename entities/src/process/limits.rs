const KIB: u64 = 1024;
const MIB: u64 = 1024 * KIB;
const GIB: u64 = 1024 * MIB;

#[must_use]
pub fn parse_memory_limit(raw: &str) -> Option<u64> {
    let trimmed = raw.trim();
    let (digits, unit) = split_unit(trimmed);
    let amount: u64 = digits.trim().parse().ok()?;
    let bytes = amount.checked_mul(unit_size(unit)?)?;
    let kib = bytes / KIB;
    (kib > 0).then_some(kib)
}

#[must_use]
pub const fn breaches_memory_limit(limit_kib: u64, rss_kib: u64) -> bool {
    rss_kib > limit_kib
}

fn split_unit(raw: &str) -> (&str, &str) {
    let boundary = raw
        .char_indices()
        .find(|(_, letter)| !letter.is_ascii_digit() && !letter.is_whitespace())
        .map_or(raw.len(), |(index, _)| index);
    raw.split_at(boundary)
}

fn unit_size(unit: &str) -> Option<u64> {
    match unit.trim().to_ascii_uppercase().as_str() {
        "" | "B" => Some(1),
        "K" | "KB" | "KIB" => Some(KIB),
        "M" | "MB" | "MIB" => Some(MIB),
        "G" | "GB" | "GIB" => Some(GIB),
        _ => None,
    }
}

#[cfg(test)]
#[path = "../tests/process_limits_tests.rs"]
mod tests;
