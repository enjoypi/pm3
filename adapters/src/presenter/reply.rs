use usecases::StartOutcome;

use super::{describe::render_describe, fields::format_pid, table::render_table};
use crate::state::DaemonReply;

pub const NOTHING_STARTED: &str = "no apps were started";

#[must_use]
pub fn already_running_marker(name: &str) -> String {
    format!("{name} is already running")
}

#[must_use]
pub fn render_reply(reply: &DaemonReply) -> String {
    match reply {
        DaemonReply::Started(outcomes) => render_started(outcomes),
        DaemonReply::Listed(views) => render_table(views),
        DaemonReply::Described(view) => render_describe(view),
        DaemonReply::Stopped { name } => format!("stopped {name}"),
        DaemonReply::Restarted { name } => format!("restarted {name}"),
        DaemonReply::Deleted { name } => format!("deleted {name}"),
    }
}

#[must_use]
pub fn render_started(outcomes: &[StartOutcome]) -> String {
    if outcomes.is_empty() {
        return NOTHING_STARTED.to_string();
    }
    let lines: Vec<String> = outcomes.iter().map(describe_start).collect();
    lines.join("\n")
}

fn describe_start(outcome: &StartOutcome) -> String {
    let StartOutcome {
        pm_id,
        name,
        pid,
        already_running,
    } = outcome;
    let pid_text = format_pid(*pid);
    if *already_running {
        return format!(
            "{} (id {pm_id}, pid {pid_text})",
            already_running_marker(name)
        );
    }
    format!("started {name} (id {pm_id}, pid {pid_text})")
}

#[cfg(test)]
#[path = "../tests/presenter_reply_tests.rs"]
mod tests;
