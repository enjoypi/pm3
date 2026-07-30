use std::{collections::BTreeMap, fmt::Write as _};

use super::file::{AppEntry, AppsFile, AppsFileError, SandboxEntry};
use crate::program::fold_home;

const ENV_SEPARATOR: char = '=';
const REMOVED_PREFIX: char = '-';
const ADDED_PREFIX: char = '+';
const ENTRY_INDENT: &str = "    ";
const NESTED_INDENT: &str = "      ";

pub struct InlineRequest<'r> {
    pub name: &'r str,
    pub program: &'r str,
    pub args: &'r [String],
    pub cwd: Option<&'r str>,
    pub home: Option<&'r str>,
    pub env: &'r [String],
    pub network: bool,
    pub writable_dirs: &'r [String],
}

pub fn inline_apps_file(request: &InlineRequest<'_>) -> Result<AppsFile, AppsFileError> {
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
        .map(|value| fold_home(value, request.home))
        .collect();
    let entry = AppEntry {
        name: request.name.to_string(),
        script: fold_home(request.program, request.home),
        cwd,
        args,
        env,
        depends_on: Vec::new(),
        autorestart: None,
        min_uptime_ms: None,
        max_restarts: None,
        restart_delay_ms: None,
        sandbox: Some(sandbox),
    };
    Ok(AppsFile { apps: vec![entry] })
}

#[must_use]
pub fn encode_apps_file(apps: &AppsFile) -> String {
    let mut text = String::from("apps:\n");
    for entry in &apps.apps {
        text.push_str(&encode_entry(entry));
    }
    text
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
    let mut text = format!("  - name: {}\n", quote(&entry.name));
    text.push_str(&scalar("script", &quote(&entry.script)));
    text.push_str(&optional_text("cwd", entry.cwd.as_deref()));
    text.push_str(&sequence(ENTRY_INDENT, "args", &entry.args));
    text.push_str(&mapping(&entry.env));
    text.push_str(&sequence(ENTRY_INDENT, "depends_on", &entry.depends_on));
    text.push_str(&optional("autorestart", entry.autorestart));
    text.push_str(&optional("min_uptime_ms", entry.min_uptime_ms));
    text.push_str(&optional("max_restarts", entry.max_restarts));
    text.push_str(&optional("restart_delay_ms", entry.restart_delay_ms));
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
    format!("{ENTRY_INDENT}sandbox:\n{text}")
}

fn scalar(key: &str, value: &str) -> String {
    format!("{ENTRY_INDENT}{key}: {value}\n")
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
        .fold(format!("{ENTRY_INDENT}env:\n"), |mut text, (key, value)| {
            let _ = writeln!(text, "{NESTED_INDENT}{}: {}", quote(key), quote(value));
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
    let mut roots: Vec<String> = cwd.map(ToString::to_string).into_iter().collect();
    for dir in extra {
        let folded = fold_home(dir, home);
        if !roots.iter().any(|known| known == &folded) {
            roots.push(folded);
        }
    }
    Some(roots)
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
