use std::{
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use super::InstallError;

const DIRECTORY_MODE: u32 = 0o700;
const FILE_MODE: u32 = 0o600;
const INCOMING_SUFFIX: &str = ".incoming";

pub async fn back_up(paths: &[PathBuf], root: &Path, stamp: &str) -> Result<PathBuf, InstallError> {
    let dir = root.join(stamp);
    prepare_dir(root).await?;
    prepare_dir(&dir).await?;
    for path in paths {
        copy_into(path, &dir).await?;
    }
    Ok(dir)
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
    tokio::fs::rename(&staged, destination)
        .await
        .map_err(|error| InstallError::replace_io(destination, &error))
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

async fn restrict_dir(path: &Path) -> Result<(), InstallError> {
    tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(DIRECTORY_MODE))
        .await
        .map_err(|error| InstallError::backup_directory(path, &error))
}

async fn restrict_file(path: &Path) -> Result<(), InstallError> {
    tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(FILE_MODE))
        .await
        .map_err(|error| InstallError::backup_io(path, &error))
}

fn staged_path(destination: &Path) -> PathBuf {
    let mut staged = destination.as_os_str().to_owned();
    staged.push(INCOMING_SUFFIX);
    PathBuf::from(staged)
}

#[cfg(test)]
#[path = "../tests/install_store_tests.rs"]
mod tests;
