use std::{
    path::{Path, PathBuf},
    process::Output,
};

use thiserror::Error;
use tokio::process::Command;

use super::{
    command::{ServiceCommand, ServiceProgramSet},
    plan::{ServiceStep, status_command},
    spec::{ServiceStatus, ServiceUnitSpec, parse_run_state},
};
use crate::exit_status::{describe_refusal, exit_code_of};

#[derive(Debug, Error)]
pub enum ServiceCommandError {
    #[error("cannot run '{program}': {reason}")]
    Spawn { program: String, reason: String },

    #[error("cannot complete '{program}': {reason}")]
    Failed { program: String, reason: String },

    #[error("cannot write '{path}': {reason}")]
    Io { path: String, reason: String },
}

pub async fn execute_plan(steps: &[ServiceStep]) -> Result<Vec<String>, ServiceCommandError> {
    let mut skipped = Vec::new();
    let mut created = Vec::new();
    for step in steps {
        created.extend(about_to_create(step).await);
        match run_step(step).await {
            Ok(None) => {}
            Ok(Some(note)) => skipped.push(note),
            Err(error) => {
                roll_back(&created).await;
                return Err(error);
            }
        }
    }
    Ok(skipped)
}

async fn about_to_create(step: &ServiceStep) -> Option<PathBuf> {
    let ServiceStep::Write { path, .. } = step else {
        return None;
    };
    match tokio::fs::try_exists(path).await {
        Ok(false) => Some(path.clone()),
        Ok(true) => None,
        Err(_unreadable) => None,
    }
}

async fn roll_back(created: &[PathBuf]) {
    for path in created.iter().rev() {
        let removed = tokio::fs::remove_file(path).await.is_ok();
        log_roll_back(path, removed);
    }
}

fn log_roll_back(path: &Path, removed: bool) {
    let file = path.to_string_lossy();
    tracing::warn!(
        file = %file,
        removed,
        action = "service",
        "pm3 backed out a file it had written before the plan failed",
    );
}

pub async fn query_status(
    spec: &ServiceUnitSpec,
    programs: &ServiceProgramSet,
) -> Result<ServiceStatus, ServiceCommandError> {
    if !spec.unit_path().is_file() {
        return Ok(ServiceStatus::NotInstalled);
    }
    let captured = capture(&status_command(spec, programs)).await?;
    if parse_run_state(spec.kind, captured.success, &captured.stdout) {
        return Ok(ServiceStatus::Running);
    }
    Ok(ServiceStatus::InstalledNotRunning)
}

async fn run_step(step: &ServiceStep) -> Result<Option<String>, ServiceCommandError> {
    match step {
        ServiceStep::Write {
            dir,
            path,
            contents,
        } => write_file(dir, path, contents).await.map(|()| None),
        ServiceStep::Remove { path } => remove_path(path).await.map(|()| None),
        ServiceStep::Run(command) => run_command(command).await.map(|()| None),
        ServiceStep::TryRun(command) => Ok(tolerate(command).await),
    }
}

async fn tolerate(command: &ServiceCommand) -> Option<String> {
    let Err(error) = run_command(command).await else {
        return None;
    };
    let note = error.to_string();
    let program = command.program.as_str();
    tracing::warn!(
        program,
        action = "service",
        "skipped an optional service manager command"
    );
    Some(note)
}

async fn write_file(dir: &Path, path: &Path, contents: &str) -> Result<(), ServiceCommandError> {
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(|error| io_error(dir, &error))?;
    tokio::fs::write(path, contents)
        .await
        .map_err(|error| io_error(path, &error))
}

async fn remove_path(path: &Path) -> Result<(), ServiceCommandError> {
    tokio::fs::remove_file(path)
        .await
        .map_err(|error| io_error(path, &error))
}

async fn run_command(command: &ServiceCommand) -> Result<(), ServiceCommandError> {
    let captured = capture(command).await?;
    if captured.success {
        return Ok(());
    }
    Err(ServiceCommandError::Failed {
        program: command.program.clone(),
        reason: describe_refusal(&captured.stderr, captured.code),
    })
}

async fn capture(command: &ServiceCommand) -> Result<Captured, ServiceCommandError> {
    let output = Command::new(&command.program)
        .args(&command.args)
        .output()
        .await
        .map_err(|error| ServiceCommandError::Spawn {
            program: command.program.clone(),
            reason: error.to_string(),
        })?;
    let captured = Captured::from_output(&output);
    let program = command.program.as_str();
    let code = captured.code;
    tracing::debug!(
        program,
        code,
        action = "service",
        "ran a service manager command"
    );
    Ok(captured)
}

fn io_error(path: &Path, source: &std::io::Error) -> ServiceCommandError {
    ServiceCommandError::Io {
        path: path.to_string_lossy().into_owned(),
        reason: source.to_string(),
    }
}

struct Captured {
    success: bool,
    stdout: String,
    stderr: String,
    code: i32,
}

impl Captured {
    fn from_output(output: &Output) -> Self {
        Self {
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            code: exit_code_of(&output.status),
        }
    }
}

#[cfg(test)]
#[path = "../tests/service_runner_tests.rs"]
mod tests;
