use std::{path::Path, process::Output};

use thiserror::Error;
use tokio::process::Command;

use super::{
    command::{ServiceCommand, ServiceProgramSet},
    plan::{ServiceStep, status_command},
    spec::{ServiceStatus, ServiceUnitSpec, parse_run_state},
};

const UNKNOWN_EXIT_CODE: i32 = -1;

#[derive(Debug, Error)]
pub enum ServiceCommandError {
    #[error("cannot run '{program}': {reason}")]
    Spawn { program: String, reason: String },

    #[error("cannot complete '{program}': {reason}")]
    Failed { program: String, reason: String },

    #[error("cannot write '{path}': {reason}")]
    Io { path: String, reason: String },
}

pub async fn execute_plan(steps: &[ServiceStep]) -> Result<(), ServiceCommandError> {
    for step in steps {
        run_step(step).await?;
    }
    Ok(())
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

async fn run_step(step: &ServiceStep) -> Result<(), ServiceCommandError> {
    match step {
        ServiceStep::Write {
            dir,
            path,
            contents,
        } => write_file(dir, path, contents).await,
        ServiceStep::Remove { path } => remove_path(path).await,
        ServiceStep::Run(command) => run_command(command).await,
    }
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

fn describe_refusal(stderr: &str, code: i32) -> String {
    let trimmed = stderr.trim();
    if trimmed.is_empty() {
        return format!("exited with status {code}");
    }
    trimmed.to_string()
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
            code: output.status.code().unwrap_or(UNKNOWN_EXIT_CODE),
        }
    }
}

#[cfg(test)]
#[path = "../tests/service_runner_tests.rs"]
mod tests;
