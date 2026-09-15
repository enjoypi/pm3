use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
};

use futures_util::future::join_all;
use tokio::fs;
use usecases::{
    DumpContents, DumpError, DumpStore, ProcessRecord, ProcessRuntime, ServiceSnapshot,
    StrandedProcess,
};

use super::dto::{DecodeError, DumpDocument, StateDto, decode_state, encode_states};
use crate::apps_file::{AppsFileError, SpecSource};

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

    async fn rejoin(&self, state: StateDto) -> Result<Rejoined, DumpError> {
        let runtime = match decode_state(state.clone()) {
            Ok(runtime) => runtime,
            Err(error) => {
                warn_undecodable(&error);
                return Ok(Rejoined::Stranded(stranded_of(&state)));
            }
        };
        match self.specs.resolve_service(&runtime.name).await {
            Ok(spec) => Ok(Rejoined::Record(Box::new(ProcessRecord { spec, runtime }))),
            Err(AppsFileError::EncFile(error)) => {
                let reason = error.to_string();
                warn_unreadable_environment(&runtime.name, &reason);
                Err(DumpError::Unreadable {
                    path: self.path.to_string_lossy().into_owned(),
                    reason,
                })
            }
            Err(error) => {
                warn_unusable(&runtime.name, &error);
                Ok(Rejoined::Stranded(stranded_from(runtime)))
            }
        }
    }
}

enum Rejoined {
    Record(Box<ProcessRecord>),
    Stranded(StrandedProcess),
}

fn stranded_of(state: &StateDto) -> StrandedProcess {
    StrandedProcess {
        name: state.name.clone(),
        pid: state.runtime.pid,
        token: state
            .runtime
            .identity
            .as_ref()
            .map(|identity| identity.token.clone()),
    }
}

fn stranded_from(runtime: ProcessRuntime) -> StrandedProcess {
    StrandedProcess {
        name: runtime.name,
        pid: runtime.pid,
        token: runtime.identity.map(|identity| identity.token),
    }
}

impl DumpStore for YamlDumpStore {
    async fn load(&self) -> Result<DumpContents, DumpError> {
        let Some(raw) = read_optional(&self.path).await? else {
            return Ok(DumpContents::default());
        };
        let doc: DumpDocument =
            serde_yaml2::from_str(&raw).map_err(|e| read_error(&self.path, &e.to_string()))?;
        let rejoined = join_all(doc.services.into_iter().map(|state| self.rejoin(state))).await;
        let mut contents = DumpContents {
            records: Vec::with_capacity(rejoined.len()),
            stranded: Vec::new(),
            boot: doc.boot,
        };
        for outcome in rejoined {
            match outcome? {
                Rejoined::Record(record) => contents.records.push(*record),
                Rejoined::Stranded(orphan) => contents.stranded.push(orphan),
            }
        }
        Ok(contents)
    }

    async fn save(&self, records: &[ProcessRecord], boot: Option<&str>) -> Result<(), DumpError> {
        let yaml = serde_yaml2::to_string(encode_states(records, boot))
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

pub async fn dump_snapshot(path: &Path) -> Result<Vec<ServiceSnapshot>, DumpError> {
    let Some(raw) = read_optional(path).await? else {
        return Ok(Vec::new());
    };
    let doc: DumpDocument =
        serde_yaml2::from_str(&raw).map_err(|e| read_error(path, &e.to_string()))?;
    Ok(doc.services.into_iter().map(snapshot_of).collect())
}

fn snapshot_of(state: StateDto) -> ServiceSnapshot {
    ServiceSnapshot {
        name: state.name,
        pid: state.runtime.pid,
    }
}

async fn write_atomically(path: &Path, contents: &str) -> Result<(), DumpError> {
    let staged = staging_path(path);
    crate::private_file::write_private(&staged, contents)
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

fn warn_unreadable_environment(app: &str, reason: &str) {
    tracing::warn!(
        feature = "persistence",
        action = "rejoin",
        app,
        reason,
        "pm3 stops reading the state file rather than letting an unreadable environment evict a running app",
    );
}

fn warn_undecodable(error: &DecodeError) {
    let reason = error.to_string();
    let app = match error {
        DecodeError::UnknownStatus { app, status: _ }
        | DecodeError::InconsistentState { app, source: _ } => app.as_str(),
    };
    tracing::warn!(
        feature = "persistence",
        action = "rejoin",
        app,
        reason,
        "pm3 cannot restore a saved app from the state file",
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
