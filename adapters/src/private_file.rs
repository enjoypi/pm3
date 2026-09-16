#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::{io::Result, path::Path};

use tokio::{
    fs::{File, OpenOptions},
    io::AsyncWriteExt as _,
};

pub const OWNER_ONLY_FILE: u32 = 0o600;
pub const SWEEP_PROOF_FILE: u32 = 0o1600;

pub async fn write_private(path: &Path, contents: &str) -> Result<()> {
    let file = private_options()
        .write(true)
        .truncate(true)
        .open(path)
        .await?;
    fill(file, contents.as_bytes()).await
}

pub async fn write_sweep_proof(path: &Path, contents: &str) -> Result<()> {
    write_private(path, contents).await?;
    set_sweep_proof(path).await
}

#[cfg(unix)]
async fn set_sweep_proof(path: &Path) -> Result<()> {
    let permissions = std::fs::Permissions::from_mode(SWEEP_PROOF_FILE);
    tokio::fs::set_permissions(path, permissions).await
}

#[cfg(not(unix))]
#[expect(
    clippy::unused_async,
    reason = "NTFS has no sticky bit; the signature stays uniform across platforms"
)]
async fn set_sweep_proof(_path: &Path) -> Result<()> {
    Ok(())
}

async fn fill(mut file: File, contents: &[u8]) -> Result<()> {
    let written = file.write_all(contents).await;
    written.and(file.flush().await)
}

pub async fn append_private(path: &Path) -> Result<File> {
    private_options().append(true).open(path).await
}

pub fn append_private_blocking(path: &Path) -> Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    options.mode(OWNER_ONLY_FILE);
    options.open(path)
}

fn private_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    options.create(true);
    #[cfg(unix)]
    options.mode(OWNER_ONLY_FILE);
    options
}

#[cfg(test)]
#[path = "tests/private_file_tests.rs"]
mod tests;
