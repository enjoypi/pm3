use usecases::ProcessView;

use super::fields::{format_list, format_pid, format_sandbox, format_uptime, pad, widest};

const LABEL_GAP: &str = "  ";

#[must_use]
pub fn render_describe(view: &ProcessView) -> String {
    let rows = describe_rows(view);
    let width = widest(rows.iter().map(|(label, _value)| label.chars().count()));
    let lines: Vec<String> = rows
        .iter()
        .map(|(label, value)| format!("{}{LABEL_GAP}{value}", pad(label, width)))
        .collect();
    lines.join("\n")
}

fn describe_rows(view: &ProcessView) -> Vec<(&'static str, String)> {
    vec![
        ("id", view.pm_id.to_string()),
        ("name", view.name.clone()),
        ("status", view.status.as_str().to_string()),
        ("pid", format_pid(view.pid)),
        ("uptime", format_uptime(view.uptime_ms)),
        ("restarts", view.restart_time.to_string()),
        ("script", view.script.clone()),
        ("args", format_list(&view.args)),
        ("cwd", view.cwd.clone()),
        ("depends on", format_list(&view.depends_on)),
        (
            "sandbox",
            format_sandbox(&view.sandbox_mode, view.sandbox_network),
        ),
        ("writable roots", format_list(&view.writable_roots)),
    ]
}

#[cfg(test)]
#[path = "../tests/presenter_describe_tests.rs"]
mod tests;
