use std::{
    mem,
    path::{Path, PathBuf},
};

use thiserror::Error;
use tokio::{fs::File, io::AsyncReadExt};

#[derive(Debug, Error)]
#[error("cannot read log file '{path}': {reason}")]
pub struct LogReadError {
    pub path: String,
    pub reason: String,
}

#[derive(Debug)]
pub struct LogFollower {
    path: PathBuf,
    file: File,
    pending: String,
}

#[must_use]
pub fn tail_lines(content: &str, count: usize) -> Vec<&str> {
    let all: Vec<&str> = content.lines().collect();
    let skipped = all.len().saturating_sub(count);
    all.into_iter().skip(skipped).collect()
}

pub async fn read_tail(path: &Path, count: usize) -> Result<Vec<String>, LogReadError> {
    let content = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| read_error(path, &e.to_string()))?;
    Ok(tail_lines(&content, count)
        .into_iter()
        .map(str::to_string)
        .collect())
}

impl LogFollower {
    pub async fn start_at_end(path: &Path) -> Result<Self, LogReadError> {
        let mut file = File::open(path)
            .await
            .map_err(|e| read_error(path, &e.to_string()))?;
        let mut skipped = String::new();
        file.read_to_string(&mut skipped)
            .await
            .map_err(|e| read_error(path, &e.to_string()))?;
        Ok(Self {
            path: path.to_path_buf(),
            file,
            pending: String::new(),
        })
    }

    pub async fn poll_appended(&mut self) -> Result<Vec<String>, LogReadError> {
        let mut chunk = String::new();
        self.file
            .read_to_string(&mut chunk)
            .await
            .map_err(|e| read_error(&self.path, &e.to_string()))?;
        self.pending.push_str(&chunk);
        Ok(self.take_complete_lines())
    }

    fn take_complete_lines(&mut self) -> Vec<String> {
        let mut lines = Vec::new();
        while let Some(index) = self.pending.find('\n') {
            let rest = self.pending.split_off(index + 1);
            let mut line = mem::replace(&mut self.pending, rest);
            line.pop();
            lines.push(line);
        }
        lines
    }
}

fn read_error(path: &Path, reason: &str) -> LogReadError {
    LogReadError {
        path: path.to_string_lossy().into_owned(),
        reason: reason.to_string(),
    }
}

#[cfg(test)]
#[path = "../tests/logs_tail_tests.rs"]
mod tests;
