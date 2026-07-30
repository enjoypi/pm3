pub const STDOUT_SUFFIX: &str = "-out.log";
pub const STDERR_SUFFIX: &str = "-err.log";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogPaths {
    pub stdout: String,
    pub stderr: String,
}

#[must_use]
pub fn log_paths(logs_dir: &str, app: &str) -> LogPaths {
    LogPaths {
        stdout: join_log_path(logs_dir, app, STDOUT_SUFFIX),
        stderr: join_log_path(logs_dir, app, STDERR_SUFFIX),
    }
}

fn join_log_path(logs_dir: &str, app: &str, suffix: &str) -> String {
    let trimmed = logs_dir.trim_end_matches('/');
    format!("{trimmed}/{app}{suffix}")
}

#[cfg(test)]
#[path = "tests/log_paths_tests.rs"]
mod tests;
