use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};

use super::InstallError;

const DEFAULT_DESTINATION: &str = "bin/pm3";
const BACKUP_DIRECTORY: &str = "install-backups";

pub fn destination_of(
    declared: Option<&str>,
    home_env: Option<&str>,
) -> Result<PathBuf, InstallError> {
    match declared {
        Some(path) if !path.is_empty() => Ok(PathBuf::from(path)),
        _ => home_env
            .map(|home| Path::new(home).join(DEFAULT_DESTINATION))
            .ok_or(InstallError::DestinationHome),
    }
}

#[must_use]
pub fn backup_root(declared: Option<&str>, pm3_home: &Path) -> PathBuf {
    match declared {
        Some(path) if !path.is_empty() => PathBuf::from(path),
        _ => pm3_home.join(BACKUP_DIRECTORY),
    }
}

#[must_use]
pub fn backup_stamp(now: DateTime<Utc>) -> String {
    now.format("%Y%m%dT%H%M%SZ").to_string()
}

#[cfg(test)]
#[path = "../tests/install_layout_tests.rs"]
mod tests;
