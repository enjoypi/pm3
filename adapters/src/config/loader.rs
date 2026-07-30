use std::{env, fs};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigLoadError {
    #[error("cannot read config file '{path}': {source}")]
    IoError {
        path: String,
        source: std::io::Error,
    },

    #[error("cannot decode environment variable '{name}': contains non-UTF-8 bytes")]
    EnvVarNotUnicode { name: String },

    #[error(
        "cannot resolve environment variable '{name}': not set and placeholder declares no default"
    )]
    EnvVarNotSet { name: String },

    #[error(
        "cannot substitute environment variable '{name}': value contains '{character}', which would change the YAML document structure"
    )]
    EnvVarNotYamlSafe { name: String, character: String },
}

type EnvLookup = fn(&str) -> Result<Option<String>, ConfigLoadError>;

pub fn load_config(path: &str) -> Result<String, ConfigLoadError> {
    fs::read_to_string(path).map_err(|e| ConfigLoadError::IoError {
        path: path.to_string(),
        source: e,
    })
}

pub fn substitute_env_vars(raw: &str) -> Result<String, ConfigLoadError> {
    substitute_with(raw, lookup_process_env)
}

fn ensure_yaml_safe(name: &str, value: &str) -> Result<(), ConfigLoadError> {
    let offending = value
        .chars()
        .find(|c| *c == '"' || *c == '\\' || c.is_control());
    let Some(character) = offending else {
        return Ok(());
    };
    Err(ConfigLoadError::EnvVarNotYamlSafe {
        name: name.to_string(),
        character: character.escape_default().to_string(),
    })
}

fn lookup_process_env(name: &str) -> Result<Option<String>, ConfigLoadError> {
    classify_env_var(name, env::var(name))
}

fn classify_env_var(
    name: &str,
    read: Result<String, env::VarError>,
) -> Result<Option<String>, ConfigLoadError> {
    match read {
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => Err(ConfigLoadError::EnvVarNotUnicode {
            name: name.to_string(),
        }),
    }
}

#[expect(
    clippy::string_slice,
    clippy::single_match_else,
    reason = "match→closure 转换让 llvm-cov 算独立未覆盖 fn；切片边界均为 ASCII"
)]
fn substitute_with(raw: &str, lookup: EnvLookup) -> Result<String, ConfigLoadError> {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let body = &rest[start + 2..];
        match parse_placeholder(body) {
            Some(parsed) => {
                let value = match lookup(parsed.name)? {
                    Some(found) => {
                        ensure_yaml_safe(parsed.name, &found)?;
                        found
                    }
                    None => match parsed.default {
                        Some(default) => default.to_string(),
                        None => {
                            return Err(ConfigLoadError::EnvVarNotSet {
                                name: parsed.name.to_string(),
                            });
                        }
                    },
                };
                out.push_str(&value);
                rest = &body[parsed.consumed..];
            }
            None => {
                out.push_str("${");
                rest = body;
            }
        }
    }
    out.push_str(rest);
    Ok(out)
}

struct ParsedPlaceholder<'a> {
    name: &'a str,
    default: Option<&'a str>,
    consumed: usize,
}

#[expect(
    clippy::option_if_let_else,
    clippy::string_slice,
    reason = "closure 形态会被 llvm-cov 当作独立未覆盖 fn；切片边界均为 ASCII"
)]
fn parse_placeholder(body: &str) -> Option<ParsedPlaceholder<'_>> {
    let line_end = match body.find('\n') {
        Some(p) => p,
        None => body.len(),
    };
    let line = &body[..line_end];
    let close = find_unnested_close(line)?;
    let inner = &line[..close];
    let (name, default) = match inner.find(":-") {
        Some(p) => (&inner[..p], Some(&inner[p + 2..])),
        None => (inner, None),
    };
    if name.is_empty() || name.contains(':') || name.contains('{') {
        return None;
    }
    Some(ParsedPlaceholder {
        name,
        default,
        consumed: close + 1,
    })
}

fn find_unnested_close(line: &str) -> Option<usize> {
    let mut depth = 0_usize;
    for (index, byte) in line.bytes().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' if depth == 0 => return Some(index),
            b'}' => depth -= 1,
            _ => {}
        }
    }
    None
}

#[cfg(test)]
#[path = "../test_helpers/config_test_helpers.rs"]
mod test_helpers;
#[cfg(test)]
#[path = "../tests/config_tests.rs"]
mod tests;
