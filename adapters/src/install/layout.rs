use std::path::{Path, PathBuf};

use usecases::is_name_letter;

use super::InstallError;

const DEFAULT_DESTINATION: &str = "bin/pm3";
const BACKUP_DIRECTORY: &str = "install-backups";
const UNKNOWN_VERSION: &str = "unknown";

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
pub fn backup_name(version: Option<&str>) -> String {
    match version {
        Some(version) if is_usable(version) => version.to_string(),
        _ => UNKNOWN_VERSION.to_string(),
    }
}

#[must_use]
pub fn parse_version_output(stdout: &str) -> Option<&str> {
    let token = stdout.split_whitespace().next_back()?;
    is_usable(token).then_some(token)
}

fn is_usable(version: &str) -> bool {
    !version.is_empty() && version.chars().all(is_name_letter)
}

#[cfg(test)]
#[path = "../tests/install_layout_tests.rs"]
mod tests;
