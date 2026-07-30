use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
};

use tokio::fs;
use usecases::{DumpError, DumpStore, ProcessRecord};

use super::dto::{DumpDocument, decode_records, encode_records};

const TMP_SUFFIX: &str = ".tmp";

#[derive(Clone, Debug)]
pub struct YamlDumpStore {
    path: PathBuf,
}

impl YamlDumpStore {
    #[must_use]
    pub const fn new(path: PathBuf) -> Self {
        Self { path }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        self.path.as_path()
    }
}

impl DumpStore for YamlDumpStore {
    async fn load(&self) -> Result<Vec<ProcessRecord>, DumpError> {
        let Some(raw) = read_optional(&self.path).await? else {
            return Ok(Vec::new());
        };
        let doc: DumpDocument =
            serde_yaml2::from_str(&raw).map_err(|e| read_error(&self.path, &e.to_string()))?;
        decode_records(doc).map_err(|e| read_error(&self.path, &e.to_string()))
    }

    async fn save(&self, records: &[ProcessRecord]) -> Result<(), DumpError> {
        let yaml = serde_yaml2::to_string(encode_records(records))
            .expect("internal error: DumpDocument serialization is infallible");
        write_atomically(&self.path, &yaml).await
    }
}

async fn read_optional(path: &Path) -> Result<Option<String>, DumpError> {
    match fs::read_to_string(path).await {
        Ok(raw) => Ok(Some(raw)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(read_error(path, &e.to_string())),
    }
}

async fn write_atomically(path: &Path, contents: &str) -> Result<(), DumpError> {
    let staged = staging_path(path);
    fs::write(&staged, contents)
        .await
        .map_err(|e| write_error(&staged, &e.to_string()))?;
    fs::rename(&staged, path)
        .await
        .map_err(|e| write_error(path, &e.to_string()))
}

fn staging_path(path: &Path) -> PathBuf {
    let mut staged = path.as_os_str().to_os_string();
    staged.push(TMP_SUFFIX);
    PathBuf::from(staged)
}

fn read_error(path: &Path, reason: &str) -> DumpError {
    DumpError::Read {
        path: path.to_string_lossy().into_owned(),
        reason: reason.to_string(),
    }
}

fn write_error(path: &Path, reason: &str) -> DumpError {
    DumpError::Write {
        path: path.to_string_lossy().into_owned(),
        reason: reason.to_string(),
    }
}

#[cfg(test)]
#[path = "../tests/persistence_yaml_store_tests.rs"]
mod tests;
