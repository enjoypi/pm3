use std::{
    collections::BTreeMap,
    io,
    os::unix::fs::PermissionsExt as _,
    path::{Path, PathBuf},
    str::Chars,
    time::Instant,
};

use thiserror::Error;
use usecases::{SpecError, validate_app_name};

pub const ENV_FILE_SUFFIX: &str = "env";

const SECRET_FILE_MODE: u32 = 0o600;
const COMMENT_PREFIX: char = '#';
const PAIR_SEPARATOR: char = '=';
const DOUBLE_FENCE: char = '"';
const SINGLE_FENCE: char = '\'';
const ESCAPE_PREFIX: char = '\\';
const HEX_MARKER: char = 'x';
const HEX_RADIX: u32 = 16;
const HEX_DIGITS: usize = 2;
const BRACED_HOME: &str = "${HOME}";
const BARE_HOME: &str = "$HOME";

#[derive(Debug, Error)]
pub enum EnvFileError {
    #[error("cannot read the environment file '{path}': {reason}")]
    Read { path: String, reason: String },

    #[error("cannot parse the environment file '{path}' at line {line}: expected KEY=VALUE")]
    Malformed { path: String, line: usize },

    #[error(
        "cannot accept the key '{key}' in the environment file '{path}' at line {line}: use letters, digits and '_', and do not start with a digit"
    )]
    UnsafeKey {
        path: String,
        key: String,
        line: usize,
    },

    #[error(
        "cannot accept the key '{key}' twice in the environment file '{path}': line {line} repeats it"
    )]
    DuplicateKey {
        path: String,
        key: String,
        line: usize,
    },
}

pub fn env_file_of(cfg_dir: &Path, name: &str) -> Result<PathBuf, SpecError> {
    validate_app_name(name)?;
    Ok(cfg_dir.join(format!("{name}.{ENV_FILE_SUFFIX}")))
}

pub async fn load_env_file(
    path: &Path,
    home: Option<&str>,
) -> Result<Vec<(String, String)>, EnvFileError> {
    let shown = path.to_string_lossy().into_owned();
    let Some(text) = read_optional(path, &shown).await? else {
        return Ok(Vec::new());
    };
    secure_file(path, &shown).await;
    parse_env_file(&shown, home, &text)
}

pub fn parse_env_file(
    path: &str,
    home: Option<&str>,
    text: &str,
) -> Result<Vec<(String, String)>, EnvFileError> {
    let mut parsed: BTreeMap<String, String> = BTreeMap::new();
    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with(COMMENT_PREFIX) {
            continue;
        }
        let (key, value) = split_pair(path, home, line, trimmed)?;
        if parsed.insert(key.clone(), value).is_some() {
            return Err(EnvFileError::DuplicateKey {
                path: path.to_string(),
                key,
                line,
            });
        }
    }
    Ok(parsed.into_iter().collect())
}

async fn read_optional(path: &Path, shown: &str) -> Result<Option<String>, EnvFileError> {
    match tokio::fs::read_to_string(path).await {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(EnvFileError::Read {
            path: shown.to_string(),
            reason: error.to_string(),
        }),
    }
}

async fn secure_file(path: &Path, shown: &str) {
    if is_linked(path).await {
        log_spared_link(shown);
        return;
    }
    let started = Instant::now();
    let permissions = std::fs::Permissions::from_mode(SECRET_FILE_MODE);
    let tightened = tokio::fs::set_permissions(path, permissions).await;
    let duration_ms = started.elapsed().as_millis();
    match tightened {
        Ok(()) => log_secured(shown, duration_ms),
        Err(error) => log_stuck_permissions(shown, &error.to_string()),
    }
}

async fn is_linked(path: &Path) -> bool {
    tokio::fs::symlink_metadata(path)
        .await
        .is_ok_and(|entry| entry.file_type().is_symlink())
}

fn split_pair(
    path: &str,
    home: Option<&str>,
    line: usize,
    text: &str,
) -> Result<(String, String), EnvFileError> {
    let Some((raw_key, raw_value)) = text.split_once(PAIR_SEPARATOR) else {
        return Err(EnvFileError::Malformed {
            path: path.to_string(),
            line,
        });
    };
    let key = raw_key.trim().to_string();
    if !is_env_key(&key) {
        return Err(EnvFileError::UnsafeKey {
            path: path.to_string(),
            key,
            line,
        });
    }
    Ok((key, unquote(home, raw_value.trim())))
}

fn is_env_key(key: &str) -> bool {
    let mut letters = key.chars();
    letters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && letters.all(|letter| letter.is_ascii_alphanumeric() || letter == '_')
}

fn unquote(home: Option<&str>, raw: &str) -> String {
    if let Some(inner) = fenced(raw, SINGLE_FENCE) {
        return inner.to_string();
    }
    let plain = fenced(raw, DOUBLE_FENCE).map_or_else(|| raw.to_string(), unescape);
    let Some(home) = home else {
        return plain;
    };
    expand_bare_home(home, &plain.replace(BRACED_HOME, home))
}

fn expand_bare_home(home: &str, text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some((before, after)) = rest.split_once(BARE_HOME) {
        out.push_str(before);
        out.push_str(if continues_a_name(after) {
            BARE_HOME
        } else {
            home
        });
        rest = after;
    }
    out.push_str(rest);
    out
}

fn continues_a_name(rest: &str) -> bool {
    rest.starts_with(|letter: char| letter.is_ascii_alphanumeric() || letter == '_')
}

fn fenced(raw: &str, fence: char) -> Option<&str> {
    raw.strip_prefix(fence)?.strip_suffix(fence)
}

fn unescape(inner: &str) -> String {
    let mut out = String::with_capacity(inner.len());
    let mut letters = inner.chars();
    while let Some(letter) = letters.next() {
        if letter == ESCAPE_PREFIX {
            push_unescaped(&mut out, &mut letters);
        } else {
            out.push(letter);
        }
    }
    out
}

fn push_unescaped(out: &mut String, letters: &mut Chars<'_>) {
    match letters.next() {
        Some('n') => out.push('\n'),
        Some('r') => out.push('\r'),
        Some('t') => out.push('\t'),
        Some(HEX_MARKER) => push_decoded_hex(out, letters),
        Some(other) => out.push(other),
        None => out.push(ESCAPE_PREFIX),
    }
}

fn push_decoded_hex(out: &mut String, letters: &mut Chars<'_>) {
    let digits: String = letters.take(HEX_DIGITS).collect();
    let decoded = decode_hex(&digits);
    out.push_str(&decoded.map_or_else(|| verbatim_hex(&digits), String::from));
}

fn verbatim_hex(digits: &str) -> String {
    format!("{ESCAPE_PREFIX}{HEX_MARKER}{digits}")
}

fn decode_hex(digits: &str) -> Option<char> {
    if digits.len() != HEX_DIGITS {
        return None;
    }
    char::from_u32(u32::from_str_radix(digits, HEX_RADIX).ok()?)
}

fn log_secured(path: &str, duration_ms: u128) {
    tracing::debug!(
        feature = "service",
        action = "secure_env_file",
        path,
        duration_ms,
        "pm3 tightened an environment file to owner-only",
    );
}

fn log_spared_link(path: &str) {
    tracing::warn!(
        feature = "service",
        action = "secure_env_file",
        path,
        "pm3 left a linked environment file alone, so its target keeps the permissions its owner chose",
    );
}

fn log_stuck_permissions(path: &str, reason: &str) {
    tracing::warn!(
        feature = "service",
        action = "secure_env_file",
        path,
        reason,
        "pm3 cannot tighten an environment file, so its values stay readable by other users",
    );
}

#[cfg(test)]
#[path = "../tests/apps_file_env_file_tests.rs"]
mod tests;
