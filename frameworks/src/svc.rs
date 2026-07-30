use std::path::{Path, PathBuf};

use adapters::{
    AppsFile, InlineRequest, diff_lines, encode_apps_file, fold_home, fold_svc_cwd,
    inline_apps_file, load_apps_file, resolve_program, service_file_of,
};

use crate::{Error, Result};

pub const MISSING_COMMAND: &str = "--name needs a program to run after it";
pub const AMBIGUOUS_TARGET: &str =
    "without --name, start takes exactly one apps file; pm3 options must come before the program";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Reconciled {
    Unchanged,
    Stale,
}

pub struct InlineStart<'s> {
    pub name: &'s str,
    pub target: &'s [String],
    pub cwd: Option<&'s str>,
    pub env: &'s [String],
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
) -> Result<PathBuf> {
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
    let apps = inline_apps_file(&InlineRequest {
        name: request.name,
        program,
        args,
        cwd: request.cwd,
        home: context.home,
        env: request.env,
        network: request.network,
        writable_dirs: request.writable_dirs,
    })?;
    let contents = encode_apps_file(&apps);
    let path = service_file_of(context.cfg_dir, request.name);
    write_svc(&path, &contents, request.force).await?;
    Ok(path)
}

pub async fn split_apps_file(context: &SvcContext<'_>, apps_file: &str, force: bool) -> Result<()> {
    let apps = load_apps_file(apps_file)?;
    for entry in &apps.apps {
        let mut folded = entry.clone();
        folded.script = fold_home(&folded.script, context.home);
        folded.cwd = folded.cwd.map(|value| fold_home(&value, context.home));
        folded.args = folded
            .args
            .iter()
            .map(|value| fold_svc_cwd(&fold_home(value, context.home)))
            .collect();
        let single = AppsFile { apps: vec![folded] };
        let contents = encode_apps_file(&single);
        write_svc(
            &service_file_of(context.cfg_dir, &entry.name),
            &contents,
            force,
        )
        .await?;
    }
    Ok(())
}

pub async fn forget(cfg_dir: &Path, name: &str) {
    tokio::fs::remove_file(service_file_of(cfg_dir, name))
        .await
        .ok();
}

pub async fn reconcile(path: &Path, contents: &str, force: bool) -> Result<Reconciled> {
    let existing = tokio::fs::read_to_string(path).await.unwrap_or_default();
    if existing == contents {
        return Ok(Reconciled::Unchanged);
    }
    if existing.is_empty() || force {
        return Ok(Reconciled::Stale);
    }
    Err(Error::SvcConflict {
        path: path.to_string_lossy().into_owned(),
        diff: diff_lines(&existing, contents).join("\n"),
    })
}

async fn write_svc(path: &Path, contents: &str, force: bool) -> Result<()> {
    if reconcile(path, contents, force).await? == Reconciled::Unchanged {
        return Ok(());
    }
    tokio::fs::write(path, contents)
        .await
        .map_err(|error| Error::SvcWrite {
            path: path.to_string_lossy().into_owned(),
            reason: error.to_string(),
        })
}

#[cfg(test)]
#[path = "tests/svc_tests.rs"]
mod tests;
