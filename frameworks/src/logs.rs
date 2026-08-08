use std::{ffi::OsStr, path::Path, time::Duration};

use adapters::{
    LogFollower, LogStream, Pm3Config, Pm3Paths, SERVICE_FILE_SUFFIX, clear_log, log_path,
    read_tail, validate_app_name,
};

use crate::{
    Result,
    commands::{Session, open_session},
};

pub const FOLLOW_FOREVER: u32 = u32::MAX;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LogRequest {
    pub names: Vec<String>,
    pub lines: Option<usize>,
    pub err: bool,
    pub all: bool,
    pub follow: bool,
    pub action: LogAction,
    pub polls: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LogAction {
    #[default]
    Show,
    Clear,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogTarget {
    pub name: String,
    pub stream: LogStream,
    pub path: String,
    pub prefix: String,
}

pub async fn run_logs(
    config: &str,
    request: &LogRequest,
    emit: &(dyn Fn(&str) + Send + Sync),
) -> Result<Option<String>> {
    let session = open_session(config)?;
    let targets = resolve_targets(&session, &request.names, request.err, request.all)?;
    let strict = targets.len() == 1 && !request.all;
    if request.action == LogAction::Clear {
        return clear_targets(&targets, strict).await.map(Some);
    }
    let count = request
        .lines
        .unwrap_or_else(|| log_tail_lines(&session.config.pm3));
    let tail = read_tails(&targets, count, strict).await?;
    if !request.follow {
        return Ok(Some(tail));
    }
    emit(&tail);
    follow_targets(&session, &targets, strict, request.polls, emit).await?;
    Ok(None)
}

pub fn log_file(paths: &Pm3Paths, name: &str, stream: LogStream) -> Result<String> {
    validate_app_name(name)?;
    Ok(log_path(&paths.logs_dir.to_string_lossy(), name, stream))
}

fn resolve_targets(
    session: &Session,
    names: &[String],
    err: bool,
    all: bool,
) -> Result<Vec<LogTarget>> {
    let names = resolve_names(session, names)?;
    let verbatim = names.len() == 1 && !all;
    let logs_dir = session.paths.logs_dir.to_string_lossy();
    let mut targets = Vec::new();
    for name in &names {
        for stream in streams_of(err, all) {
            targets.push(LogTarget {
                name: name.clone(),
                stream,
                path: log_path(&logs_dir, name, stream),
                prefix: prefix_of(name, stream, verbatim, all),
            });
        }
    }
    Ok(targets)
}

fn resolve_names(session: &Session, names: &[String]) -> Result<Vec<String>> {
    if names.is_empty() {
        return Ok(declared_service_names(&session.cfg_dir));
    }
    let mut resolved = Vec::new();
    for name in names {
        validate_app_name(name)?;
        resolved.push(name.clone());
    }
    Ok(resolved)
}

fn declared_service_names(cfg_dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(cfg_dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(std::result::Result::ok)
        .filter_map(|entry| service_name_of(&entry.file_name()))
        .collect();
    names.sort_unstable();
    names
}

fn service_name_of(file_name: &OsStr) -> Option<String> {
    let text = file_name.to_str()?;
    let stem = text.strip_suffix(&format!(".{SERVICE_FILE_SUFFIX}"))?;
    validate_app_name(stem).ok()?;
    Some(stem.to_string())
}

fn streams_of(err: bool, all: bool) -> Vec<LogStream> {
    if all {
        vec![LogStream::Stdout, LogStream::Stderr]
    } else if err {
        vec![LogStream::Stderr]
    } else {
        vec![LogStream::Stdout]
    }
}

fn prefix_of(name: &str, stream: LogStream, verbatim: bool, all: bool) -> String {
    if verbatim {
        String::new()
    } else if all {
        format!("{} [{}] | ", name, stream_tag(stream))
    } else {
        format!("{name} | ")
    }
}

const fn stream_tag(stream: LogStream) -> &'static str {
    match stream {
        LogStream::Stdout => "out",
        LogStream::Stderr => "err",
    }
}

fn log_tail_lines(pm3: &Pm3Config) -> usize {
    usize::try_from(pm3.log_tail_lines).unwrap_or(usize::MAX)
}

async fn read_tails(targets: &[LogTarget], count: usize, strict: bool) -> Result<String> {
    let mut output = String::new();
    for target in targets {
        let lines = match read_tail(Path::new(&target.path), count).await {
            Ok(lines) => lines,
            Err(error) if strict => return Err(error.into()),
            Err(error) => {
                log_skipped_target(target, &error.to_string());
                continue;
            }
        };
        append_lines(&mut output, target, &lines);
    }
    Ok(output)
}

fn append_lines(output: &mut String, target: &LogTarget, lines: &[String]) {
    for line in lines {
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str(&target.prefix);
        output.push_str(line);
    }
}

async fn clear_targets(targets: &[LogTarget], strict: bool) -> Result<String> {
    let mut output = String::new();
    for target in targets {
        match clear_log(Path::new(&target.path)).await {
            Ok(()) => append_cleared(&mut output, &target.path),
            Err(error) if strict => return Err(error.into()),
            Err(error) => log_skipped_target(target, &error.to_string()),
        }
    }
    Ok(output)
}

fn append_cleared(output: &mut String, path: &str) {
    if !output.is_empty() {
        output.push('\n');
    }
    output.push_str("cleared ");
    output.push_str(path);
}

struct ActiveFollower {
    prefix: String,
    follower: LogFollower,
}

async fn follow_targets(
    session: &Session,
    targets: &[LogTarget],
    strict: bool,
    polls: u32,
    emit: &(dyn Fn(&str) + Send + Sync),
) -> Result<()> {
    let interval = Duration::from_millis(session.config.pm3.log_follow_interval_ms);
    let mut followers = open_followers(targets, strict).await?;
    for _poll in 0..polls {
        for active in &mut followers {
            for line in active.follower.poll_appended().await? {
                emit(&format!("{}{line}", active.prefix));
            }
        }
        tokio::time::sleep(interval).await;
    }
    Ok(())
}

async fn open_followers(targets: &[LogTarget], strict: bool) -> Result<Vec<ActiveFollower>> {
    let mut followers = Vec::new();
    for target in targets {
        let path = Path::new(&target.path);
        let opened = if strict {
            Some(LogFollower::start_at_end(path).await?)
        } else {
            LogFollower::start_at_end_if_exists(path).await?
        };
        let Some(follower) = opened else {
            log_skipped_target(target, "the log file does not exist");
            continue;
        };
        followers.push(ActiveFollower {
            prefix: target.prefix.clone(),
            follower,
        });
    }
    Ok(followers)
}

fn log_skipped_target(target: &LogTarget, reason: &str) {
    let app = target.name.as_str();
    let path = target.path.as_str();
    tracing::debug!(
        feature = "client",
        action = "log_skip",
        app,
        path,
        reason,
        "pm3 logs skips a service without a readable log file",
    );
}

#[cfg(test)]
#[path = "tests/logs_tests.rs"]
mod tests;
