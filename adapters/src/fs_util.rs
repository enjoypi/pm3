use std::{
    io,
    path::{Path, PathBuf},
};

#[must_use]
pub fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut extended = path.as_os_str().to_owned();
    extended.push(suffix);
    PathBuf::from(extended)
}

pub async fn read_optional(path: &Path) -> io::Result<Option<String>> {
    match tokio::fs::read_to_string(path).await {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

pub async fn remove_if_present(path: &Path) -> io::Result<()> {
    match tokio::fs::remove_file(path).await {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

pub async fn truncate(path: &Path) -> io::Result<()> {
    tokio::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(path)
        .await
        .map(drop)
}

#[cfg(test)]
#[path = "tests/fs_util_tests.rs"]
mod tests;
