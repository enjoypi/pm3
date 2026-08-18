use std::fmt::Write as _;

use super::{
    file::{AppEntry, ReadyProbeEntry, SandboxEntry},
    roots::dedup_roots,
};
use crate::program::{HOME_PLACEHOLDER, SERVICE_CWD_PLACEHOLDER, fold_home, fold_service_cwd};

const REMOVED_PREFIX: char = '-';
const ADDED_PREFIX: char = '+';

const NESTED_INDENT: &str = "  ";

pub struct InlineRequest<'r> {
    pub name: &'r str,
    pub program: &'r str,
    pub args: &'r [String],
    pub cwd: Option<&'r str>,
    pub home: Option<&'r str>,
    pub cron: Option<&'r str>,
    pub autorestart: Option<bool>,
    pub network: bool,
    pub writable_dirs: &'r [String],
    pub readable_dirs: &'r [String],
    pub max_memory: Option<&'r str>,
    pub ready_exec: &'r [String],
    pub ready_tcp: Option<&'r str>,
    pub listen_timeout_ms: Option<u64>,
    pub stop_exit_codes: &'r [i32],
}

#[must_use]
pub fn inline_entry(request: &InlineRequest<'_>) -> AppEntry {
    let sandbox = SandboxEntry {
        mode: None,
        read: None,
        network: request.network.then_some(true),
        writable_roots: (!request.writable_dirs.is_empty()).then(|| request.writable_dirs.to_vec()),
        readable_roots: (!request.readable_dirs.is_empty()).then(|| request.readable_dirs.to_vec()),
    };
    let entry = AppEntry {
        name: request.name.to_string(),
        script: request.program.to_string(),
        cwd: request.cwd.map(ToString::to_string),
        args: request.args.to_vec(),
        rejected_env: None,
        depends_on: Vec::new(),
        autorestart: request.autorestart,
        min_uptime_ms: None,
        max_restarts: None,
        restart_delay_ms: None,
        max_restart_delay_ms: None,
        schedule: request.cron.map(ToString::to_string),
        max_memory: request.max_memory.map(ToString::to_string),
        stop_exit_codes: request.stop_exit_codes.to_vec(),
        listen_timeout_ms: request.listen_timeout_ms,
        ready_probe: ready_probe_of(request),
        sandbox: Some(sandbox),
    };
    fold_entry(&entry, request.home)
}

fn ready_probe_of(request: &InlineRequest<'_>) -> Option<ReadyProbeEntry> {
    if !request.ready_exec.is_empty() {
        return Some(ReadyProbeEntry {
            exec: Some(request.ready_exec.to_vec()),
            tcp: None,
        });
    }
    request.ready_tcp.map(|endpoint| ReadyProbeEntry {
        exec: None,
        tcp: Some(endpoint.to_string()),
    })
}

#[must_use]
pub fn fold_entry(entry: &AppEntry, home: Option<&str>) -> AppEntry {
    let mut folded = entry.clone();
    folded.script = fold_home(&folded.script, home);
    folded.cwd = folded.cwd.map(|value| fold_home(&value, home));
    folded.args = folded
        .args
        .iter()
        .map(|value| fold_service_cwd(&fold_home(value, home)))
        .collect();
    if let Some(sandbox) = folded.sandbox.as_mut() {
        sandbox.writable_roots = sandbox
            .writable_roots
            .as_ref()
            .map(|roots| fold_roots(roots, home));
        sandbox.readable_roots = sandbox
            .readable_roots
            .as_ref()
            .map(|roots| fold_roots(roots, home));
    }
    folded
}

fn fold_roots(roots: &[String], home: Option<&str>) -> Vec<String> {
    dedup_roots(roots.iter().map(|root| fold_home(root, home)))
}

#[must_use]
pub fn encode_service_file(entry: &AppEntry) -> String {
    encode_entry(entry)
}

#[must_use]
pub fn diff_lines(old: &str, new: &str) -> Vec<String> {
    let before: Vec<&str> = old.lines().collect();
    let after: Vec<&str> = new.lines().collect();
    let mut diff = Vec::new();
    for index in 0..before.len().max(after.len()) {
        let removed = before.get(index);
        let added = after.get(index);
        if removed == added {
            continue;
        }
        if let Some(line) = removed {
            diff.push(format!("{REMOVED_PREFIX}{line}"));
        }
        if let Some(line) = added {
            diff.push(format!("{ADDED_PREFIX}{line}"));
        }
    }
    diff
}

fn encode_entry(entry: &AppEntry) -> String {
    let mut text = scalar("name", &quote(&entry.name));
    text.push_str(&scalar("script", &quote_placeheld(&entry.script)));
    text.push_str(&placeheld_text("cwd", entry.cwd.as_deref()));
    text.push_str(&placeheld_sequence("", "args", &entry.args));
    text.push_str(&sequence("", "depends_on", &entry.depends_on));
    text.push_str(&optional("autorestart", entry.autorestart));
    text.push_str(&optional("min_uptime_ms", entry.min_uptime_ms));
    text.push_str(&optional("max_restarts", entry.max_restarts));
    text.push_str(&optional("restart_delay_ms", entry.restart_delay_ms));
    text.push_str(&optional(
        "max_restart_delay_ms",
        entry.max_restart_delay_ms,
    ));
    text.push_str(&optional_text("schedule", entry.schedule.as_deref()));
    text.push_str(&optional_text("max_memory", entry.max_memory.as_deref()));
    text.push_str(&number_sequence("stop_exit_codes", &entry.stop_exit_codes));
    text.push_str(&optional("listen_timeout_ms", entry.listen_timeout_ms));
    text.push_str(&encode_ready_probe(entry.ready_probe.as_ref()));
    text.push_str(&encode_sandbox(entry.sandbox.as_ref()));
    text
}

fn encode_ready_probe(probe: Option<&ReadyProbeEntry>) -> String {
    let Some(section) = probe else {
        return String::new();
    };
    let exec = section.exec.as_deref().unwrap_or_default();
    let mut text = sequence(NESTED_INDENT, "exec", exec);
    if let Some(endpoint) = section.tcp.as_deref() {
        text.push_str(&nested("tcp", &quote(endpoint)));
    }
    if text.is_empty() {
        return text;
    }
    format!("ready_probe:\n{text}")
}

fn encode_sandbox(sandbox: Option<&SandboxEntry>) -> String {
    let Some(section) = sandbox else {
        return String::new();
    };
    let mode = section.mode.as_deref().map(quote);
    let read = section.read.as_deref().map(quote);
    let writable = section.writable_roots.as_deref().unwrap_or_default();
    let readable = section.readable_roots.as_deref().unwrap_or_default();
    let mut text = String::new();
    if let Some(quoted) = mode {
        text.push_str(&nested("mode", &quoted));
    }
    if let Some(quoted) = read {
        text.push_str(&nested("read", &quoted));
    }
    if let Some(network) = section.network {
        text.push_str(&nested("network", &network.to_string()));
    }
    text.push_str(&placeheld_sequence(
        NESTED_INDENT,
        "writable_roots",
        writable,
    ));
    text.push_str(&placeheld_sequence(
        NESTED_INDENT,
        "readable_roots",
        readable,
    ));
    if text.is_empty() {
        return text;
    }
    format!("sandbox:\n{text}")
}

fn scalar(key: &str, value: &str) -> String {
    format!("{key}: {value}\n")
}

fn nested(key: &str, value: &str) -> String {
    format!("{NESTED_INDENT}{key}: {value}\n")
}

fn optional_text(key: &str, value: Option<&str>) -> String {
    value.map_or_else(String::new, |shown| scalar(key, &quote(shown)))
}

fn placeheld_text(key: &str, value: Option<&str>) -> String {
    value.map_or_else(String::new, |shown| scalar(key, &quote_placeheld(shown)))
}

fn optional<T: std::fmt::Display>(key: &str, value: Option<T>) -> String {
    value.map_or_else(String::new, |shown| scalar(key, &shown.to_string()))
}

fn sequence(indent: &str, key: &str, values: &[String]) -> String {
    write_sequence(indent, key, values, quote)
}

fn placeheld_sequence(indent: &str, key: &str, values: &[String]) -> String {
    write_sequence(indent, key, values, quote_placeheld)
}

fn write_sequence(
    indent: &str,
    key: &str,
    values: &[String],
    quoted: fn(&str) -> String,
) -> String {
    if values.is_empty() {
        return String::new();
    }
    values
        .iter()
        .fold(format!("{indent}{key}:\n"), |mut text, value| {
            let _ = writeln!(text, "{indent}  - {}", quoted(value));
            text
        })
}

fn number_sequence(key: &str, values: &[i32]) -> String {
    if values.is_empty() {
        return String::new();
    }
    values.iter().fold(format!("{key}:\n"), |mut text, value| {
        let _ = writeln!(text, "  - {value}");
        text
    })
}

fn quote(raw: &str) -> String {
    format!("\"{}\"", escape_body(raw))
}

fn quote_placeheld(raw: &str) -> String {
    let mut text = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some((head, placeholder, tail)) = next_placeholder(rest) {
        text.push_str(&escape_body(head));
        text.push_str(placeholder);
        rest = tail;
    }
    text.push_str(&escape_body(rest));
    format!("\"{text}\"")
}

fn next_placeholder(rest: &str) -> Option<(&str, &'static str, &str)> {
    [HOME_PLACEHOLDER, SERVICE_CWD_PLACEHOLDER]
        .into_iter()
        .filter_map(|placeholder| {
            rest.split_once(placeholder)
                .map(|(head, tail)| (head, placeholder, tail))
        })
        .min_by_key(|(head, _placeholder, _tail)| head.len())
}

fn escape_body(raw: &str) -> String {
    let mut escaped = String::with_capacity(raw.len());
    for ch in raw.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            '$' => push_hex_escape(&mut escaped, ch),
            ch if ch.is_control() => push_hex_escape(&mut escaped, ch),
            ch => escaped.push(ch),
        }
    }
    escaped
}

fn push_hex_escape(escaped: &mut String, ch: char) {
    let _ = write!(escaped, "\\x{:02x}", ch as u32);
}

#[cfg(test)]
#[path = "../tests/apps_file_inline_tests.rs"]
mod tests;
