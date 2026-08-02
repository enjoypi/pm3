pub const DAEMON_NOT_RUNNING: &str = "the pm3 daemon is not running";

#[must_use]
pub fn render_daemon_gone(stopped: Option<&str>) -> String {
    stopped.map_or_else(
        || DAEMON_NOT_RUNNING.to_string(),
        |services| format!("{services}\n{DAEMON_NOT_RUNNING}"),
    )
}

#[must_use]
pub fn render_daemon_stopped(stopped: Option<&str>, pid: u32) -> String {
    let farewell = format!("stopped the pm3 daemon (pid {pid})");
    stopped.map_or_else(
        || format!("{farewell}; managed services keep running"),
        |services| format!("{services}\n{farewell}"),
    )
}

#[cfg(test)]
#[path = "../tests/presenter_daemon_tests.rs"]
mod tests;
