use std::io::{BufRead, Write};

#[must_use]
pub fn stale_running<'n>(changed: &'n [String], already_running: &[String]) -> Vec<&'n str> {
    changed
        .iter()
        .filter(|name| already_running.contains(name))
        .map(String::as_str)
        .collect()
}

pub fn confirm_restart(name: &str, input: &mut impl BufRead, output: &mut impl Write) -> bool {
    let _ = write!(
        output,
        "config changed for '{name}'; restart to apply? [y/N] "
    )
    .and_then(|()| output.flush());
    let mut answer = String::new();
    input.read_line(&mut answer).unwrap_or_default();
    if !answer.ends_with('\n') {
        writeln!(output).ok();
    }
    matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

#[must_use]
pub fn keep_old_config_hint(name: &str) -> String {
    format!(
        "'{name}' keeps running with the previous config; run 'pm3 restart {name}' to apply the new one"
    )
}

#[cfg(test)]
#[path = "tests/prompt_tests.rs"]
mod tests;
