pub const STDOUT_SUFFIX: &str = "-out.log";
pub const STDERR_SUFFIX: &str = "-err.log";

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum LogStream {
    Stdout,
    Stderr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogPaths {
    pub stdout: String,
    pub stderr: String,
}

#[must_use]
pub fn log_paths(logs_dir: &str, app: &str) -> LogPaths {
    LogPaths {
        stdout: log_path(logs_dir, app, LogStream::Stdout),
        stderr: log_path(logs_dir, app, LogStream::Stderr),
    }
}

#[must_use]
pub fn log_path(logs_dir: &str, app: &str, stream: LogStream) -> String {
    let suffix = match stream {
        LogStream::Stdout => STDOUT_SUFFIX,
        LogStream::Stderr => STDERR_SUFFIX,
    };
    join_log_path(logs_dir, app, suffix)
}

fn join_log_path(logs_dir: &str, app: &str, suffix: &str) -> String {
    let trimmed = logs_dir.trim_end_matches('/');
    format!("{trimmed}/{app}{suffix}")
}

#[cfg(test)]
#[path = "tests/log_paths_tests.rs"]
mod tests;
