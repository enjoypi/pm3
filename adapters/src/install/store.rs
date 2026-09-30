#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use super::InstallError;
#[cfg(unix)]
use crate::private_file::{OWNER_ONLY_DIR, OWNER_ONLY_FILE};

const INCOMING_SUFFIX: &str = ".incoming";
#[cfg(windows)]
const RETIRED_SUFFIX: &str = ".retired";

pub async fn back_up(paths: &[PathBuf], root: &Path, stamp: &str) -> Result<PathBuf, InstallError> {
    let dir = root.join(stamp);
    prepare_dir(root).await?;
    prepare_dir(&dir).await?;
    for path in paths {
        copy_into(path, &dir).await?;
    }
    Ok(dir)
}

pub async fn binary_matches(source: &Path, destination: &Path) -> Result<bool, InstallError> {
    let incoming = tokio::fs::read(source)
        .await
        .map_err(|error| InstallError::replace_io(source, &error))?;
    match tokio::fs::read(destination).await {
        Ok(installed) => Ok(incoming == installed),
        Err(error) => {
            if error.kind() == std::io::ErrorKind::NotFound {
                return Ok(false);
            }
            if occupied_by_directory(destination).await {
                return Ok(false);
            }
            Err(InstallError::replace_io(destination, &error))
        }
    }
}

async fn occupied_by_directory(path: &Path) -> bool {
    tokio::fs::metadata(path)
        .await
        .as_ref()
        .is_ok_and(std::fs::Metadata::is_dir)
}

pub async fn replace_binary(source: &Path, destination: &Path) -> Result<(), InstallError> {
    if let Some(parent) = destination.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| InstallError::replace_io(parent, &error))?;
    }
    let staged = staged_path(destination);
    tokio::fs::copy(source, &staged)
        .await
        .map_err(|error| InstallError::replace_io(source, &error))?;
    #[cfg(windows)]
    retire_current(destination).await?;
    tokio::fs::rename(&staged, destination)
        .await
        .map_err(|error| InstallError::replace_io(destination, &error))
}

#[cfg(windows)]
async fn retire_current(destination: &Path) -> Result<(), InstallError> {
    let retired = retired_path(destination);
    crate::fs_util::remove_if_present(&retired)
        .await
        .map_err(|error| InstallError::replace_io(&retired, &error))?;
    match tokio::fs::try_exists(destination).await {
        Ok(true) => tokio::fs::rename(destination, &retired)
            .await
            .map_err(|error| InstallError::replace_io(destination, &error)),
        Ok(false) => Ok(()),
        Err(error) => Err(InstallError::replace_io(destination, &error)),
    }
}

#[cfg(windows)]
fn retired_path(destination: &Path) -> PathBuf {
    crate::fs_util::with_suffix(destination, RETIRED_SUFFIX)
}

async fn prepare_dir(path: &Path) -> Result<(), InstallError> {
    tokio::fs::create_dir_all(path)
        .await
        .map_err(|error| InstallError::backup_directory(path, &error))?;
    restrict_dir(path).await
}

async fn copy_into(path: &Path, dir: &Path) -> Result<(), InstallError> {
    match tokio::fs::try_exists(path).await {
        Ok(true) => {}
        Ok(false) => return Ok(()),
        Err(error) => return Err(InstallError::backup_io(path, &error)),
    }
    let Some(name) = path.file_name() else {
        return Err(InstallError::backup(path, "has no file name".to_string()));
    };
    let target = dir.join(name);
    tokio::fs::copy(path, &target)
        .await
        .map_err(|error| InstallError::backup_io(path, &error))?;
    restrict_file(&target).await
}

#[cfg(unix)]
async fn restrict_dir(path: &Path) -> Result<(), InstallError> {
    tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(OWNER_ONLY_DIR))
        .await
        .map_err(|error| InstallError::backup_directory(path, &error))
}

#[cfg(not(unix))]
#[expect(
    clippy::unused_async,
    reason = "NTFS user directories are already per-user; the signature stays uniform"
)]
async fn restrict_dir(_path: &Path) -> Result<(), InstallError> {
    Ok(())
}

#[cfg(unix)]
async fn restrict_file(path: &Path) -> Result<(), InstallError> {
    tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(OWNER_ONLY_FILE))
        .await
        .map_err(|error| InstallError::backup_io(path, &error))
}

#[cfg(not(unix))]
#[expect(
    clippy::unused_async,
    reason = "NTFS user directories are already per-user; the signature stays uniform"
)]
async fn restrict_file(_path: &Path) -> Result<(), InstallError> {
    Ok(())
}

fn staged_path(destination: &Path) -> PathBuf {
    crate::fs_util::with_suffix(destination, INCOMING_SUFFIX)
}

#[cfg(test)]
#[path = "../tests/install_store_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "../tests/install_store_windows_tests.rs"]
mod windows_tests;
