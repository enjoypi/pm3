use std::process::ExitStatus;

use usecases::ExitOutcome;

pub const UNKNOWN_EXIT_CODE: i32 = -1;

#[must_use]
pub fn exit_code_of(status: &ExitStatus) -> i32 {
    status.code().unwrap_or(UNKNOWN_EXIT_CODE)
}

#[must_use]
pub fn exit_outcome_of(status: ExitStatus) -> ExitOutcome {
    status
        .code()
        .map_or(ExitOutcome::Signalled, ExitOutcome::Code)
}

#[must_use]
pub fn describe_refusal(stderr: &str, code: i32) -> String {
    let trimmed = stderr.trim();
    if trimmed.is_empty() {
        return format!("exited with status {code}");
    }
    trimmed.to_string()
}

#[cfg(test)]
#[path = "tests/exit_status_tests.rs"]
mod tests;
