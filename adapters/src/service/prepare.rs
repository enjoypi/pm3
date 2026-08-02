use std::path::{Path, PathBuf};

use super::store::{Reconciled, ServiceError, ServiceUndo, write_service_file};
use crate::{
    apps_file::{InlineRequest, encode_service_file, fold_entry, inline_entry, load_apps_file},
    program::resolve_program,
};

pub struct InlineStart<'s> {
    pub name: &'s str,
    pub program: &'s str,
    pub args: &'s [String],
    pub cwd: Option<&'s str>,
    pub env: &'s [String],
    pub cron: Option<&'s str>,
    pub autorestart: Option<bool>,
    pub network: bool,
    pub writable_dirs: &'s [String],
    pub force: bool,
}

pub struct ServiceContext<'c> {
    pub cfg_dir: &'c Path,
    pub search_path: &'c str,
    pub home: Option<&'c str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedService {
    pub path: PathBuf,
    pub reconciled: Reconciled,
    pub undo: ServiceUndo,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SplitApps {
    pub names: Vec<String>,
    pub changed: Vec<String>,
    pub undo: ServiceUndo,
}

pub async fn prepare_inline(
    context: &ServiceContext<'_>,
    request: &InlineStart<'_>,
) -> Result<PreparedService, ServiceError> {
    if resolve_program(request.program, Some(context.search_path)).is_none() {
        return Err(ServiceError::ProgramNotFound {
            program: request.program.to_string(),
        });
    }
    let entry = inline_entry(&InlineRequest {
        name: request.name,
        program: request.program,
        args: request.args,
        cwd: request.cwd,
        home: context.home,
        env: request.env,
        cron: request.cron,
        autorestart: request.autorestart,
        network: request.network,
        writable_dirs: request.writable_dirs,
    })?;
    let contents = encode_service_file(&entry);
    let path = crate::apps_file::service_file_of(context.cfg_dir, request.name)?;
    let mut undo = ServiceUndo::default();
    let reconciled =
        write_service_file(request.name, &path, &contents, request.force, &mut undo).await?;
    Ok(PreparedService {
        path,
        reconciled,
        undo,
    })
}

pub async fn split_apps_file(
    context: &ServiceContext<'_>,
    apps_file: &str,
    force: bool,
) -> Result<SplitApps, ServiceError> {
    let apps = load_apps_file(apps_file).await?;
    let mut split = SplitApps::default();
    for entry in &apps.apps {
        let path = crate::apps_file::service_file_of(context.cfg_dir, &entry.name)?;
        let contents = encode_service_file(&fold_entry(entry, context.home));
        split.names.push(entry.name.clone());
        match write_service_file(&entry.name, &path, &contents, force, &mut split.undo).await {
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

#[cfg(test)]
#[path = "../tests/service_prepare_tests.rs"]
mod tests;
