use std::{collections::BTreeMap, fmt::Write as _};

use super::{
    file::{AppEntry, AppsFile, AppsFileError, SandboxEntry},
    roots::dedup_roots,
};
use crate::program::{fold_home, fold_svc_cwd};

const ENV_SEPARATOR: char = '=';
const REMOVED_PREFIX: char = '-';
const ADDED_PREFIX: char = '+';

const LISTED: Layout = Layout {
    lead: "  - ",
    field: "    ",
    nested: "      ",
};
const STANDALONE: Layout = Layout {
    lead: "",
    field: "",
    nested: "  ",
};

#[derive(Copy, Clone, Debug)]
struct Layout {
    lead: &'static str,
    field: &'static str,
    nested: &'static str,
}

pub struct InlineRequest<'r> {
    pub name: &'r str,
    pub program: &'r str,
    pub args: &'r [String],
    pub cwd: Option<&'r str>,
    pub home: Option<&'r str>,
    pub env: &'r [String],
    pub cron: Option<&'r str>,
    pub autorestart: Option<bool>,
    pub network: bool,
    pub writable_dirs: &'r [String],
}

pub fn inline_entry(request: &InlineRequest<'_>) -> Result<AppEntry, AppsFileError> {
    let env = parse_env_pairs(request.env)?;
    let cwd = request.cwd.map(|value| fold_home(value, request.home));
    let sandbox = SandboxEntry {
        mode: None,
        network: request.network.then_some(true),
        writable_roots: writable_roots(cwd.as_deref(), request.writable_dirs, request.home),
    };
    let args = request
        .args
        .iter()
        .map(|value| fold_svc_cwd(&fold_home(value, request.home)))
        .collect();
    let entry = AppEntry {
        name: request.name.to_string(),
        script: fold_home(request.program, request.home),
        cwd,
        args,
        env,
        depends_on: Vec::new(),
        autorestart: request.autorestart,
        min_uptime_ms: None,
        max_restarts: None,
        restart_delay_ms: None,
        schedule: request.cron.map(ToString::to_string),
        sandbox: Some(sandbox),
    };
    Ok(entry)
}

#[must_use]
pub fn encode_apps_file(apps: &AppsFile) -> String {
    let mut text = String::from("apps:\n");
    for entry in &apps.apps {
        text.push_str(&encode_entry(entry, LISTED));
    }
    text
}

#[must_use]
pub fn encode_service_file(entry: &AppEntry) -> String {
    encode_entry(entry, STANDALONE)
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

fn encode_entry(entry: &AppEntry, layout: Layout) -> String {
    let mut text = format!("{}name: {}\n", layout.lead, quote(&entry.name));
    text.push_str(&scalar(layout, "script", &quote(&entry.script)));
    text.push_str(&optional_text(layout, "cwd", entry.cwd.as_deref()));
    text.push_str(&sequence(layout.field, "args", &entry.args));
    text.push_str(&mapping(layout, &entry.env));
    text.push_str(&sequence(layout.field, "depends_on", &entry.depends_on));
    text.push_str(&optional(layout, "autorestart", entry.autorestart));
    text.push_str(&optional(layout, "min_uptime_ms", entry.min_uptime_ms));
    text.push_str(&optional(layout, "max_restarts", entry.max_restarts));
    text.push_str(&optional(
        layout,
        "restart_delay_ms",
        entry.restart_delay_ms,
    ));
    text.push_str(&optional_text(
        layout,
        "schedule",
        entry.schedule.as_deref(),
    ));
    text.push_str(&encode_sandbox(layout, entry.sandbox.as_ref()));
    text
}

fn encode_sandbox(layout: Layout, sandbox: Option<&SandboxEntry>) -> String {
    let Some(section) = sandbox else {
        return String::new();
    };
    let mode = section.mode.as_deref().map(quote);
    let roots = section.writable_roots.as_deref().unwrap_or_default();
    let mut text = String::new();
    if let Some(quoted) = mode {
        text.push_str(&nested(layout, "mode", &quoted));
    }
    if let Some(network) = section.network {
        text.push_str(&nested(layout, "network", &network.to_string()));
    }
    text.push_str(&sequence(layout.nested, "writable_roots", roots));
    if text.is_empty() {
        return text;
    }
    format!("{}sandbox:\n{text}", layout.field)
}

fn scalar(layout: Layout, key: &str, value: &str) -> String {
    format!("{}{key}: {value}\n", layout.field)
}

fn nested(layout: Layout, key: &str, value: &str) -> String {
    format!("{}{key}: {value}\n", layout.nested)
}

fn optional_text(layout: Layout, key: &str, value: Option<&str>) -> String {
    value.map_or_else(String::new, |shown| scalar(layout, key, &quote(shown)))
}

fn optional<T: std::fmt::Display>(layout: Layout, key: &str, value: Option<T>) -> String {
    value.map_or_else(String::new, |shown| scalar(layout, key, &shown.to_string()))
}

fn sequence(indent: &str, key: &str, values: &[String]) -> String {
    if values.is_empty() {
        return String::new();
    }
    values
        .iter()
        .fold(format!("{indent}{key}:\n"), |mut text, value| {
            let _ = writeln!(text, "{indent}  - {}", quote(value));
            text
        })
}

fn mapping(layout: Layout, env: &BTreeMap<String, String>) -> String {
    if env.is_empty() {
        return String::new();
    }
    let head = format!("{}env:\n", layout.field);
    env.iter().fold(head, |mut text, (key, value)| {
        let _ = writeln!(text, "{}{}: {}", layout.nested, quote(key), quote(value));
        text
    })
}

fn quote(raw: &str) -> String {
    let escaped = raw.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

fn writable_roots(cwd: Option<&str>, extra: &[String], home: Option<&str>) -> Option<Vec<String>> {
    if extra.is_empty() {
        return None;
    }
    let declared = cwd
        .map(ToString::to_string)
        .into_iter()
        .chain(extra.iter().map(|dir| fold_home(dir, home)));
    Some(dedup_roots(declared))
}

fn parse_env_pairs(pairs: &[String]) -> Result<BTreeMap<String, String>, AppsFileError> {
    let mut parsed = BTreeMap::new();
    for pair in pairs {
        let Some((key, value)) = pair.split_once(ENV_SEPARATOR) else {
            return Err(AppsFileError::InvalidEnvPair(pair.clone()));
        };
        if key.is_empty() {
            return Err(AppsFileError::InvalidEnvPair(pair.clone()));
        }
        parsed.insert(key.to_string(), value.to_string());
    }
    Ok(parsed)
}

#[cfg(test)]
#[path = "../tests/apps_file_inline_tests.rs"]
mod tests;
