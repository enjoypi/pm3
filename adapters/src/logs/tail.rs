use std::{
    mem,
    path::{Path, PathBuf},
};

use thiserror::Error;
use tokio::{
    fs::File,
    io::{AsyncReadExt as _, AsyncSeekExt as _, SeekFrom},
};

const TAIL_CHUNK_BYTES: u64 = 64 * 1024;

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
    let mut tail: Vec<&str> = content.lines().rev().take(count).collect();
    tail.reverse();
    tail
}

pub async fn read_tail(path: &Path, count: usize) -> Result<Vec<String>, LogReadError> {
    let mut file = File::open(path)
        .await
        .map_err(|e| read_error(path, &e.to_string()))?;
    let mut start = seek_to_end(&mut file).await;
    let mut buffer = Vec::new();
    while start > 0 && line_breaks(&buffer) <= count {
        let step = TAIL_CHUNK_BYTES.min(start);
        start -= step;
        let mut chunk = read_chunk_at(&mut file, start, step)
            .await
            .map_err(|e| read_error(path, &e.to_string()))?;
        chunk.append(&mut buffer);
        buffer = chunk;
    }
    let text = String::from_utf8_lossy(&buffer);
    Ok(tail_lines(&text, count)
        .into_iter()
        .map(str::to_string)
        .collect())
}

async fn seek_to_end(file: &mut File) -> u64 {
    file.seek(SeekFrom::End(0))
        .await
        .expect("internal error: seeking to the end of an open log cannot overflow")
}

async fn read_chunk_at(file: &mut File, start: u64, step: u64) -> std::io::Result<Vec<u8>> {
    file.seek(SeekFrom::Start(start))
        .await
        .expect("internal error: seeking inside an open log cannot overflow");
    let mut chunk = Vec::new();
    file.take(step).read_to_end(&mut chunk).await?;
    Ok(chunk)
}

fn line_breaks(buffer: &[u8]) -> usize {
    buffer
        .split(|byte| *byte == b'\n')
        .count()
        .saturating_sub(1)
}

impl LogFollower {
    pub async fn start_at_end(path: &Path) -> Result<Self, LogReadError> {
        let mut file = File::open(path)
            .await
            .map_err(|e| read_error(path, &e.to_string()))?;
        seek_to_end(&mut file).await;
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
