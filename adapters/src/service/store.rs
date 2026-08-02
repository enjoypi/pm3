use std::path::{Path, PathBuf};

use thiserror::Error;
use usecases::SpecError;

use crate::apps_file::{AppsFileError, diff_lines, service_file_of};

#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("cannot find '{program}' on PATH")]
    ProgramNotFound { program: String },

    #[error("cannot write the service file '{path}': {reason}")]
    Write { path: String, reason: String },

    #[error("cannot overwrite '{path}' without --force:\n{diff}")]
    Conflict { path: String, diff: String },

    #[error(transparent)]
    Apps(#[from] AppsFileError),

    #[error(transparent)]
    Spec(#[from] SpecError),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Reconciled {
    Unchanged,
    Stale,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ServiceUndo {
    steps: Vec<UndoStep>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct UndoStep {
    service: String,
    path: PathBuf,
    restore: Restore,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Restore {
    Remove,
    Replace(String),
}

impl ServiceUndo {
    pub async fn run(&self) {
        for step in &self.steps {
            step.apply().await;
        }
    }

    pub async fn run_for(&self, services: &[String]) {
        for step in &self.steps {
            if services.contains(&step.service) {
                step.apply().await;
            }
        }
    }

    pub(super) fn remember(&mut self, service: &str, path: &Path, previous: Option<String>) {
        let restore = previous.map_or(Restore::Remove, Restore::Replace);
        self.steps.push(UndoStep {
            service: service.to_string(),
            path: path.to_path_buf(),
            restore,
        });
    }
}

impl UndoStep {
    async fn apply(&self) {
        let restored = match &self.restore {
            Restore::Remove => tokio::fs::remove_file(&self.path).await,
            Restore::Replace(previous) => tokio::fs::write(&self.path, previous).await,
        };
        match restored {
            Ok(()) => log_undo(&self.path),
            Err(error) => log_stuck_undo(&self.path, &error.to_string()),
        }
    }
}

pub async fn forget(cfg_dir: &Path, name: &str) {
    let Ok(path) = service_file_of(cfg_dir, name) else {
        return;
    };
    tokio::fs::remove_file(path).await.ok();
}

pub async fn reconcile(
    path: &Path,
    contents: &str,
    force: bool,
) -> Result<Reconciled, ServiceError> {
    let existing = tokio::fs::read_to_string(path).await.unwrap_or_default();
    reconcile_contents(path, &existing, contents, force)
}

pub(super) fn reconcile_contents(
    path: &Path,
    existing: &str,
    contents: &str,
    force: bool,
) -> Result<Reconciled, ServiceError> {
    if existing == contents {
        return Ok(Reconciled::Unchanged);
    }
    if existing.is_empty() || force {
        return Ok(Reconciled::Stale);
    }
    Err(ServiceError::Conflict {
        path: path.to_string_lossy().into_owned(),
        diff: diff_lines(existing, contents).join("\n"),
    })
}

pub(super) async fn write_service_file(
    service: &str,
    path: &Path,
    contents: &str,
    force: bool,
    undo: &mut ServiceUndo,
) -> Result<Reconciled, ServiceError> {
    let existing = tokio::fs::read_to_string(path).await.ok();
    let reconciled = reconcile_contents(
        path,
        existing.as_deref().unwrap_or_default(),
        contents,
        force,
    )?;
    if reconciled == Reconciled::Unchanged {
        return Ok(Reconciled::Unchanged);
    }
    tokio::fs::write(path, contents)
        .await
        .map_err(|error| ServiceError::Write {
            path: path.to_string_lossy().into_owned(),
            reason: error.to_string(),
        })?;
    undo.remember(service, path, existing);
    Ok(Reconciled::Stale)
}

fn log_undo(path: &Path) {
    let path = path.to_string_lossy().into_owned();
    tracing::debug!(
        feature = "service",
        action = "undo",
        path,
        "pm3 rolled a service file back because the start was refused",
    );
}

fn log_stuck_undo(path: &Path, reason: &str) {
    let path = path.to_string_lossy().into_owned();
    tracing::warn!(
        feature = "service",
        action = "undo",
        path,
        reason,
        "pm3 cannot roll a service file back, so the file on disk no longer matches what is running",
    );
}

#[cfg(test)]
#[path = "../tests/service_store_tests.rs"]
mod tests;
