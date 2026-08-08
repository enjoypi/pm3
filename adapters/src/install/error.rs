use thiserror::Error;

#[derive(Debug, Error)]
pub enum InstallError {
    #[error("cannot resolve the install destination: no HOME in the environment")]
    DestinationHome,

    #[error("cannot prepare the backup directory '{path}': {reason}")]
    BackupDirectory { path: String, reason: String },

    #[error("cannot back up '{path}': {reason}")]
    Backup { path: String, reason: String },

    #[error("cannot replace '{path}': {reason}")]
    Replace { path: String, reason: String },
}

impl InstallError {
    pub(crate) fn backup_directory(path: &std::path::Path, error: &std::io::Error) -> Self {
        Self::BackupDirectory {
            path: path.to_string_lossy().into_owned(),
            reason: error.to_string(),
        }
    }

    pub(crate) fn backup(path: &std::path::Path, reason: String) -> Self {
        Self::Backup {
            path: path.to_string_lossy().into_owned(),
            reason,
        }
    }

    pub(crate) fn backup_io(path: &std::path::Path, error: &std::io::Error) -> Self {
        Self::backup(path, error.to_string())
    }

    pub(crate) fn replace_io(path: &std::path::Path, error: &std::io::Error) -> Self {
        Self::Replace {
            path: path.to_string_lossy().into_owned(),
            reason: error.to_string(),
        }
    }
}
