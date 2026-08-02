use std::{collections::BTreeMap, fmt::Write as _};

use super::{
    file::{AppEntry, AppsFileError, SandboxEntry},
    roots::dedup_roots,
};
use crate::program::{fold_home, fold_service_cwd};

const ENV_SEPARATOR: char = '=';
const REMOVED_PREFIX: char = '-';
const ADDED_PREFIX: char = '+';

const NESTED_INDENT: &str = "  ";

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
    let sandbox = SandboxEntry {
        mode: None,
        network: request.network.then_some(true),
        writable_roots: (!request.writable_dirs.is_empty()).then(|| request.writable_dirs.to_vec()),
    };
    let entry = AppEntry {
        name: request.name.to_string(),
        script: request.program.to_string(),
        cwd: request.cwd.map(ToString::to_string),
        args: request.args.to_vec(),
        env: parse_env_pairs(request.env)?,
        depends_on: Vec::new(),
        autorestart: request.autorestart,
        min_uptime_ms: None,
        max_restarts: None,
        restart_delay_ms: None,
        schedule: request.cron.map(ToString::to_string),
        sandbox: Some(sandbox),
    };
    Ok(fold_entry(&entry, request.home))
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
    folded.env = folded
        .env
        .iter()
        .map(|(key, value)| (key.clone(), fold_home(value, home)))
        .collect();
    if let Some(sandbox) = folded.sandbox.as_mut() {
        sandbox.writable_roots = sandbox
            .writable_roots
            .as_ref()
            .map(|roots| dedup_roots(roots.iter().map(|root| fold_home(root, home))));
    }
    folded
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
    text.push_str(&scalar("script", &quote(&entry.script)));
    text.push_str(&optional_text("cwd", entry.cwd.as_deref()));
    text.push_str(&sequence("", "args", &entry.args));
    text.push_str(&mapping(&entry.env));
    text.push_str(&sequence("", "depends_on", &entry.depends_on));
    text.push_str(&optional("autorestart", entry.autorestart));
    text.push_str(&optional("min_uptime_ms", entry.min_uptime_ms));
    text.push_str(&optional("max_restarts", entry.max_restarts));
    text.push_str(&optional("restart_delay_ms", entry.restart_delay_ms));
    text.push_str(&optional_text("schedule", entry.schedule.as_deref()));
    text.push_str(&encode_sandbox(entry.sandbox.as_ref()));
    text
}

fn encode_sandbox(sandbox: Option<&SandboxEntry>) -> String {
    let Some(section) = sandbox else {
        return String::new();
    };
    let mode = section.mode.as_deref().map(quote);
    let roots = section.writable_roots.as_deref().unwrap_or_default();
    let mut text = String::new();
    if let Some(quoted) = mode {
        text.push_str(&nested("mode", &quoted));
    }
    if let Some(network) = section.network {
        text.push_str(&nested("network", &network.to_string()));
    }
    text.push_str(&sequence(NESTED_INDENT, "writable_roots", roots));
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

fn optional<T: std::fmt::Display>(key: &str, value: Option<T>) -> String {
    value.map_or_else(String::new, |shown| scalar(key, &shown.to_string()))
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

fn mapping(env: &BTreeMap<String, String>) -> String {
    if env.is_empty() {
        return String::new();
    }
    env.iter()
        .fold(String::from("env:\n"), |mut text, (key, value)| {
            let _ = writeln!(text, "{NESTED_INDENT}{}: {}", quote(key), quote(value));
            text
        })
}

fn quote(raw: &str) -> String {
    let mut escaped = String::with_capacity(raw.len() + 2);
    for ch in raw.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch if ch.is_control() => push_hex_escape(&mut escaped, ch),
            ch => escaped.push(ch),
        }
    }
    format!("\"{escaped}\"")
}

fn push_hex_escape(escaped: &mut String, ch: char) {
    let _ = write!(escaped, "\\x{:02x}", ch as u32);
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
