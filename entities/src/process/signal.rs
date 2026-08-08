use thiserror::Error;

pub const VALID_SIGNALS: [&str; 6] = ["TERM", "INT", "QUIT", "HUP", "USR1", "USR2"];

#[derive(Debug, Eq, PartialEq, Error)]
#[error("unknown signal '{raw}'; valid signals: TERM, INT, QUIT, HUP, USR1, USR2")]
pub struct SignalNameError {
    pub raw: String,
}

pub fn parse_signal_name(raw: &str) -> Result<String, SignalNameError> {
    let upper = raw.to_uppercase();
    if VALID_SIGNALS.contains(&upper.as_str()) {
        return Ok(upper);
    }
    Err(SignalNameError {
        raw: raw.to_string(),
    })
}

#[cfg(test)]
#[path = "../tests/process_signal_tests.rs"]
mod tests;
