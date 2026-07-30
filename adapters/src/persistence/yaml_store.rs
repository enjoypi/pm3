use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
};

use tokio::fs;
use usecases::{DumpError, DumpStore, ProcessRecord};

use super::dto::{DumpDocument, StateDto, decode_state, encode_states};
use crate::{
    apps_file::{AppsFileError, SpecSource},
    workspace::materialise_workspace,
};

const TMP_SUFFIX: &str = ".tmp";

#[derive(Clone, Debug)]
pub struct YamlDumpStore {
    path: PathBuf,
    specs: SpecSource,
}

impl YamlDumpStore {
    #[must_use]
    pub const fn new(path: PathBuf, specs: SpecSource) -> Self {
        Self { path, specs }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        self.path.as_path()
    }

    async fn rejoin(&self, state: StateDto) -> Result<Option<ProcessRecord>, DumpError> {
        let runtime = decode_state(state).map_err(|e| read_error(&self.path, &e.to_string()))?;
        match self.specs.resolve_service(&runtime.name) {
            Ok(mut spec) => {
                materialise_workspace(&mut spec).await;
                Ok(Some(ProcessRecord { spec, runtime }))
            }
            Err(error) => {
                warn_unusable(&runtime.name, &error);
                Ok(None)
            }
        }
    }
}

impl DumpStore for YamlDumpStore {
    async fn load(&self) -> Result<Vec<ProcessRecord>, DumpError> {
        let Some(raw) = read_optional(&self.path).await? else {
            return Ok(Vec::new());
        };
        let doc: DumpDocument =
            serde_yaml2::from_str(&raw).map_err(|e| read_error(&self.path, &e.to_string()))?;
        let mut records = Vec::with_capacity(doc.services.len());
        for state in doc.services {
            if let Some(record) = self.rejoin(state).await? {
                records.push(record);
            }
        }
        Ok(records)
    }

    async fn save(&self, records: &[ProcessRecord]) -> Result<(), DumpError> {
        let yaml = serde_yaml2::to_string(encode_states(records))
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

fn warn_unusable(app: &str, error: &AppsFileError) {
    let reason = error.to_string();
    tracing::warn!(
        feature = "persistence",
        action = "rejoin",
        app,
        reason,
        "pm3 cannot restore a saved app from its service file",
    );
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
