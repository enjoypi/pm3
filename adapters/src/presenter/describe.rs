use usecases::{EnvDisplay, ProcessView};

use super::fields::{
    MISSING, format_cpu, format_env_origin, format_list, format_memory, format_pid, format_sandbox,
    format_stamp, format_uptime, pad, widest,
};

const LABEL_GAP: &str = "  ";
const ENV_LABEL: &str = "env";

#[must_use]
pub fn render_describe(view: &ProcessView) -> String {
    let fixed = render_rows(&describe_rows(view));
    let environment = render_env_block(&view.env);
    if environment.is_empty() {
        return fixed;
    }
    format!("{fixed}\n{environment}")
}

fn render_rows(rows: &[(&'static str, String)]) -> String {
    let width = widest(rows.iter().map(|(label, _value)| label.chars().count()));
    let lines: Vec<String> = rows
        .iter()
        .map(|(label, value)| format!("{}{LABEL_GAP}{value}", pad(label, width)))
        .collect();
    lines.join("\n")
}

fn render_env_block(shown: &[EnvDisplay]) -> String {
    if shown.is_empty() {
        return String::new();
    }
    let width = widest(shown.iter().map(|entry| entry.key.chars().count()));
    let lines: Vec<String> = shown.iter().map(|entry| env_line(entry, width)).collect();
    lines.join("\n")
}

fn env_line(entry: &EnvDisplay, width: usize) -> String {
    let key = pad(&entry.key, width);
    let value = &entry.value;
    let scope = entry.scope.as_str();
    format!("{ENV_LABEL} {key}{LABEL_GAP}{value}  ({scope})")
}

fn describe_rows(view: &ProcessView) -> Vec<(&'static str, String)> {
    vec![
        ("id", view.pm_id.to_string()),
        ("name", view.name.clone()),
        ("status", view.status.as_str().to_string()),
        ("pid", format_pid(view.pid)),
        ("uptime", format_uptime(view.uptime_ms)),
        ("memory", format_memory(view.rss_kib)),
        ("cpu", format_cpu(view.cpu_tenths)),
        ("restarts", view.restart_time.to_string()),
        ("unstable restarts", view.unstable_restarts.to_string()),
        (
            "schedule",
            view.schedule.clone().unwrap_or_else(|| MISSING.to_string()),
        ),
        ("next fire", format_stamp(view.next_fire_ms)),
        ("script", view.script.clone()),
        ("args", format_list(&view.args)),
        ("cwd", view.cwd.clone()),
        ("depends on", format_list(&view.depends_on)),
        (
            "sandbox",
            format_sandbox(&view.sandbox_mode, view.sandbox_network),
        ),
        ("writable roots", format_list(&view.writable_roots)),
        (
            ENV_LABEL,
            format_env_origin(view.env_origin, view.env_declared),
        ),
    ]
}

#[cfg(test)]
#[path = "../tests/presenter_describe_tests.rs"]
mod tests;
