use usecases::{StartKind, StartOutcome};

use super::{describe::render_describe, fields::format_pid, table::render_table};
use crate::state::DaemonReply;

pub const NOTHING_STARTED: &str = "no apps were started";
pub const NOTHING_TO_STOP: &str = "no services were running";

fn already_running_marker(name: &str) -> String {
    format!("{name} is already running")
}

#[must_use]
pub fn affected_service(reply: &DaemonReply) -> Option<String> {
    use DaemonReply as Dr;

    match reply {
        Dr::Stopped { name } | Dr::Restarted { name } | Dr::Deleted { name } => Some(name.clone()),
        Dr::Started(_) | Dr::Listed(_) | Dr::Described(_) | Dr::StoppedAll { names: _ } => None,
    }
}

#[must_use]
pub fn already_running_names(reply: &DaemonReply) -> Vec<String> {
    let DaemonReply::Started(outcomes) = reply else {
        return Vec::new();
    };
    outcomes
        .iter()
        .filter(|outcome| outcome.kind == StartKind::AlreadyRunning)
        .map(|outcome| outcome.name.clone())
        .collect()
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
        DaemonReply::StoppedAll { names } => render_stopped_all(names),
    }
}

#[must_use]
pub fn render_stopped_all(names: &[String]) -> String {
    if names.is_empty() {
        return NOTHING_TO_STOP.to_string();
    }
    format!("stopped {}", names.join(", "))
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
    use StartKind as Sk;

    let StartOutcome {
        pm_id,
        name,
        pid,
        kind,
    } = outcome;
    let pid_text = format_pid(*pid);
    let headline = match kind {
        Sk::Spawned => format!("started {name}"),
        Sk::AlreadyRunning => already_running_marker(name),
        Sk::Adopted => format!("reclaimed {name}"),
        Sk::Scheduled => format!("scheduled {name}"),
    };
    format!("{headline} (id {pm_id}, pid {pid_text})")
}

#[cfg(test)]
#[path = "../tests/presenter_reply_tests.rs"]
mod tests;
