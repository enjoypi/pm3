use std::{io::Result, os::unix::fs::OpenOptionsExt as _, path::Path};

use tokio::{
    fs::{File, OpenOptions},
    io::AsyncWriteExt as _,
};

pub const OWNER_ONLY_FILE: u32 = 0o600;

pub async fn write_private(path: &Path, contents: &str) -> Result<()> {
    let mut file = private_options()
        .write(true)
        .truncate(true)
        .open(path)
        .await?;
    let written = file.write_all(contents.as_bytes()).await;
    written.and(file.flush().await)
}

pub async fn append_private(path: &Path) -> Result<File> {
    private_options().append(true).open(path).await
}

pub fn append_private_blocking(path: &Path) -> Result<std::fs::File> {
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .mode(OWNER_ONLY_FILE)
        .open(path)
}

fn private_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    options.create(true).mode(OWNER_ONLY_FILE);
    options
}

#[cfg(test)]
#[path = "tests/private_file_tests.rs"]
mod tests;
