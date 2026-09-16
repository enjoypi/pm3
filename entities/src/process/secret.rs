pub const ELIDED: &str = "..";
pub const MIN_MASKABLE_CHARS: usize = 12;

const VISIBLE_CHARS: usize = 4;

#[must_use]
pub fn mask_secret(value: &str) -> String {
    let chars = value.chars().count();
    if chars < MIN_MASKABLE_CHARS {
        return format!("{ELIDED} {chars}");
    }
    let head: String = value.chars().take(VISIBLE_CHARS).collect();
    let tail: String = value.chars().skip(chars - VISIBLE_CHARS).collect();
    format!("{head}{ELIDED}{tail} {chars}")
}

#[cfg(test)]
#[path = "../tests/process_secret_tests.rs"]
mod tests;
