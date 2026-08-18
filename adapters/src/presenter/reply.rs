use usecases::{StartKind, StartOutcome, SupervisionReply};

use super::{describe::render_describe, fields::format_pid, table::render_table};

pub const NOTHING_STARTED: &str = "no apps were started";
pub const NOTHING_TO_STOP: &str = "no services were running";

fn refused_marker(reason: &str) -> String {
    format!("cannot start the rest of the batch: {reason}")
}

fn unsaved_marker(reason: &str) -> String {
    format!("cannot record what pm3 just started: {reason}")
}

fn already_running_marker(name: &str) -> String {
    format!("{name} is already running")
}

#[must_use]
pub fn affected_service(reply: &SupervisionReply) -> Option<String> {
    use SupervisionReply as Dr;

    match reply {
        Dr::Stopped { name }
        | Dr::Restarted { name }
        | Dr::Deleted { name }
        | Dr::Reset { name }
        | Dr::Signalled { name, signal: _ } => Some(name.clone()),
        Dr::Started {
            outcomes: _,
            refused: _,
            reason: _,
            unsaved: _,
        }
        | Dr::Listed(_)
        | Dr::Described(_)
        | Dr::StoppedAll { names: _ }
        | Dr::RestartedAll { names: _ }
        | Dr::DeletedAll { names: _ }
        | Dr::ResetAll { names: _ } => None,
    }
}

#[must_use]
pub fn deleted_names(reply: &SupervisionReply) -> Vec<String> {
    let SupervisionReply::DeletedAll { names } = reply else {
        return Vec::new();
    };
    names.clone()
}

#[must_use]
pub fn already_running_names(reply: &SupervisionReply) -> Vec<String> {
    let SupervisionReply::Started {
        outcomes,
        refused: _,
        reason: _,
        unsaved: _,
    } = reply
    else {
        return Vec::new();
    };
    outcomes
        .iter()
        .filter(|outcome| outcome.kind == StartKind::AlreadyRunning)
        .map(|outcome| outcome.name.clone())
        .collect()
}

#[must_use]
pub fn refused_names(reply: &SupervisionReply) -> Vec<String> {
    let SupervisionReply::Started {
        outcomes: _,
        refused,
        reason: _,
        unsaved: _,
    } = reply
    else {
        return Vec::new();
    };
    refused.clone()
}

#[must_use]
pub fn unsaved_reason(reply: &SupervisionReply) -> Option<String> {
    let SupervisionReply::Started {
        outcomes: _,
        refused: _,
        reason: _,
        unsaved,
    } = reply
    else {
        return None;
    };
    unsaved.clone()
}

#[must_use]
pub fn render_reply(reply: &SupervisionReply) -> String {
    match reply {
        SupervisionReply::Started {
            outcomes,
            refused: _,
            reason,
            unsaved,
        } => render_started(outcomes, reason.as_deref(), unsaved.as_deref()),
        SupervisionReply::Listed(views) => render_table(views),
        SupervisionReply::Described(view) => render_describe(view),
        SupervisionReply::Stopped { name } => format!("stopped {name}"),
        SupervisionReply::Restarted { name } => format!("restarted {name}"),
        SupervisionReply::Deleted { name } => format!("deleted {name}"),
        SupervisionReply::Reset { name } => format!("reset {name}"),
        SupervisionReply::Signalled { name, signal } => format!("sent {signal} to {name}"),
        SupervisionReply::StoppedAll { names } => render_stopped_all(names),
        SupervisionReply::RestartedAll { names } => render_batch("restarted", "restart", names),
        SupervisionReply::DeletedAll { names } => render_batch("deleted", "delete", names),
        SupervisionReply::ResetAll { names } => render_batch("reset", "reset", names),
    }
}

#[must_use]
pub fn render_batch(done: &str, base: &str, names: &[String]) -> String {
    if names.is_empty() {
        return format!("no apps to {base}");
    }
    format!("{done} {}", names.join(", "))
}

#[must_use]
pub fn render_stopped_all(names: &[String]) -> String {
    if names.is_empty() {
        return NOTHING_TO_STOP.to_string();
    }
    format!("stopped {}", names.join(", "))
}

#[must_use]
pub fn render_started(
    outcomes: &[StartOutcome],
    reason: Option<&str>,
    unsaved: Option<&str>,
) -> String {
    let mut lines: Vec<String> = outcomes.iter().map(describe_start).collect();
    if lines.is_empty() {
        lines.push(NOTHING_STARTED.to_string());
    }
    if let Some(refused) = reason {
        lines.push(refused_marker(refused));
    }
    if let Some(unsaved) = unsaved {
        lines.push(unsaved_marker(unsaved));
    }
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
        Sk::Deferred => format!("queued {name} until its dependency becomes ready"),
    };
    format!("{headline} (id {pm_id}, pid {pid_text})")
}

#[cfg(test)]
#[path = "../tests/presenter_reply_tests.rs"]
mod tests;
