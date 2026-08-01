use std::path::{Path, PathBuf};

use adapters::{
    InlineRequest, diff_lines, encode_service_file, fold_entry, inline_entry, load_apps_file,
    resolve_program, service_file_of,
};

use crate::{Error, Result};

pub const MISSING_COMMAND: &str = "--name needs a program to run after it";
pub const AMBIGUOUS_TARGET: &str =
    "without --name, start takes exactly one apps file; pm3 options must come before the program";

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Reconciled {
    Unchanged,
    Stale,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedSvc {
    pub path: PathBuf,
    pub reconciled: Reconciled,
    pub undo: SvcUndo,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SplitApps {
    pub names: Vec<String>,
    pub changed: Vec<String>,
    pub undo: SvcUndo,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SvcUndo {
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

impl SvcUndo {
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

    fn remember(&mut self, service: &str, path: &Path, previous: Option<String>) {
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
        match &self.restore {
            Restore::Remove => {
                tokio::fs::remove_file(&self.path).await.ok();
            }
            Restore::Replace(previous) => {
                tokio::fs::write(&self.path, previous).await.ok();
            }
        }
        log_undo(&self.path);
    }
}

fn log_undo(path: &Path) {
    let path = path.to_string_lossy().into_owned();
    tracing::debug!(
        feature = "svc",
        action = "undo",
        path,
        "pm3 rolled a service file back because the start was refused",
    );
}

pub struct InlineStart<'s> {
    pub name: &'s str,
    pub target: &'s [String],
    pub cwd: Option<&'s str>,
    pub env: &'s [String],
    pub cron: Option<&'s str>,
    pub autorestart: Option<bool>,
    pub network: bool,
    pub writable_dirs: &'s [String],
    pub force: bool,
}

pub struct SvcContext<'c> {
    pub cfg_dir: &'c Path,
    pub search_path: &'c str,
    pub home: Option<&'c str>,
}

pub async fn prepare_inline(
    context: &SvcContext<'_>,
    request: &InlineStart<'_>,
) -> Result<PreparedSvc> {
    let Some((program, args)) = request.target.split_first() else {
        return Err(Error::InlineUsage {
            reason: MISSING_COMMAND.to_string(),
        });
    };
    if resolve_program(program, Some(context.search_path)).is_none() {
        return Err(Error::ProgramNotFound {
            program: program.clone(),
        });
    }
    let entry = inline_entry(&InlineRequest {
        name: request.name,
        program,
        args,
        cwd: request.cwd,
        home: context.home,
        env: request.env,
        cron: request.cron,
        autorestart: request.autorestart,
        network: request.network,
        writable_dirs: request.writable_dirs,
    })?;
    let contents = encode_service_file(&entry);
    let path = service_file_of(context.cfg_dir, request.name)?;
    let mut undo = SvcUndo::default();
    let reconciled = write_svc(request.name, &path, &contents, request.force, &mut undo).await?;
    Ok(PreparedSvc {
        path,
        reconciled,
        undo,
    })
}

pub async fn split_apps_file(
    context: &SvcContext<'_>,
    apps_file: &str,
    force: bool,
) -> Result<SplitApps> {
    let apps = load_apps_file(apps_file).await?;
    let mut split = SplitApps::default();
    for entry in &apps.apps {
        let path = service_file_of(context.cfg_dir, &entry.name)?;
        let contents = encode_service_file(&fold_entry(entry, context.home));
        split.names.push(entry.name.clone());
        match write_svc(&entry.name, &path, &contents, force, &mut split.undo).await {
            Ok(Reconciled::Stale) => split.changed.push(entry.name.clone()),
            Ok(Reconciled::Unchanged) => {}
            Err(error) => {
                split.undo.run().await;
                return Err(error);
            }
        }
    }
    Ok(split)
}

pub async fn forget(cfg_dir: &Path, name: &str) {
    let Ok(path) = service_file_of(cfg_dir, name) else {
        return;
    };
    tokio::fs::remove_file(path).await.ok();
}

pub async fn reconcile(path: &Path, contents: &str, force: bool) -> Result<Reconciled> {
    let existing = tokio::fs::read_to_string(path).await.unwrap_or_default();
    reconcile_contents(path, &existing, contents, force)
}

fn reconcile_contents(
    path: &Path,
    existing: &str,
    contents: &str,
    force: bool,
) -> Result<Reconciled> {
    if existing == contents {
        return Ok(Reconciled::Unchanged);
    }
    if existing.is_empty() || force {
        return Ok(Reconciled::Stale);
    }
    Err(Error::SvcConflict {
        path: path.to_string_lossy().into_owned(),
        diff: diff_lines(existing, contents).join("\n"),
    })
}

async fn write_svc(
    service: &str,
    path: &Path,
    contents: &str,
    force: bool,
    undo: &mut SvcUndo,
) -> Result<Reconciled> {
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
        .map_err(|error| Error::SvcWrite {
            path: path.to_string_lossy().into_owned(),
            reason: error.to_string(),
        })?;
    undo.remember(service, path, existing);
    Ok(Reconciled::Stale)
}

#[cfg(test)]
#[path = "tests/svc_tests.rs"]
mod tests;
