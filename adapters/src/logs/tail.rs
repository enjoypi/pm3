#[cfg(unix)]
use std::os::unix::fs::MetadataExt as _;
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
    offset: u64,
    pending: Vec<u8>,
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
        let follower = Self::start_at_end_if_exists(path).await?;
        follower.ok_or_else(|| read_error(path, "the log file does not exist"))
    }

    pub async fn start_at_end_if_exists(path: &Path) -> Result<Option<Self>, LogReadError> {
        let mut file = match File::open(path).await {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(read_error(path, &error.to_string())),
        };
        let offset = seek_to_end(&mut file).await;
        Ok(Some(Self {
            path: path.to_path_buf(),
            file,
            offset,
            pending: Vec::new(),
        }))
    }

    pub async fn poll_appended(&mut self) -> Result<Vec<String>, LogReadError> {
        self.resync().await;
        let mut chunk = Vec::new();
        let read = self
            .file
            .read_to_end(&mut chunk)
            .await
            .map_err(|e| read_error(&self.path, &e.to_string()))?;
        self.offset += read as u64;
        self.pending.extend_from_slice(&chunk);
        Ok(self.take_complete_lines())
    }

    async fn resync(&mut self) {
        let meta = self
            .file
            .metadata()
            .await
            .expect("internal error: metadata of an open log file cannot fail");
        #[cfg(unix)]
        if let Some(file) = self.reopen_if_rotated(meta.ino()).await {
            self.file = file;
            self.offset = 0;
            self.pending.clear();
            return;
        }
        if meta.len() < self.offset {
            self.file
                .seek(SeekFrom::Start(0))
                .await
                .expect("internal error: seeking to the start of an open log cannot overflow");
            self.offset = 0;
            self.pending.clear();
        }
    }

    #[cfg(unix)]
    async fn reopen_if_rotated(&self, open_ino: u64) -> Option<File> {
        let Ok(meta) = tokio::fs::metadata(&self.path).await else {
            return None;
        };
        if meta.ino() == open_ino {
            return None;
        }
        File::open(&self.path).await.ok()
    }

    fn take_complete_lines(&mut self) -> Vec<String> {
        let mut lines = Vec::new();
        while let Some(index) = self.pending.iter().position(|byte| *byte == b'\n') {
            let rest = self.pending.split_off(index + 1);
            let mut line = mem::replace(&mut self.pending, rest);
            line.pop();
            lines.push(String::from_utf8_lossy(&line).into_owned());
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
