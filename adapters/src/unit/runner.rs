use std::{
    path::{Path, PathBuf},
    process::Output,
    time::Duration,
};

use thiserror::Error;
use tokio::{
    process::Command,
    time::{Instant, timeout},
};

use super::{
    command::{UnitCommand, UnitProgramSet, launchctl_kickstart, loginctl_show_linger},
    plan::{UnitStep, status_command, supervised_pid_command},
    spec::{
        LingerState, UnitKind, UnitSpec, UnitStatus, parse_launchd_pid, parse_linger_state,
        parse_main_pid, parse_run_state,
    },
};
use crate::exit_status::{describe_refusal, exit_code_of};

#[derive(Debug, Error)]
pub enum UnitCommandError {
    #[error("cannot run '{program}': {reason}")]
    Spawn { program: String, reason: String },

    #[error("cannot complete '{program}': {reason}")]
    Failed { program: String, reason: String },

    #[error("cannot write '{path}': {reason}")]
    Io { path: String, reason: String },

    #[error("cannot get an answer from '{program}' within {timeout_ms} ms")]
    Stalled { program: String, timeout_ms: u64 },
}

pub async fn execute_plan(
    steps: &[UnitStep],
    timeout_ms: u64,
) -> Result<Vec<String>, UnitCommandError> {
    let mut skipped = Vec::new();
    let mut created = Vec::new();
    for step in steps {
        let outcome = match about_to_create(step).await {
            Ok(pending) => {
                created.extend(pending);
                run_step(step, timeout_ms).await
            }
            Err(error) => Err(error),
        };
        match outcome {
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

async fn about_to_create(step: &UnitStep) -> Result<Option<PathBuf>, UnitCommandError> {
    let UnitStep::Write {
        dir: _,
        path,
        contents: _,
    } = step
    else {
        return Ok(None);
    };
    match tokio::fs::try_exists(path).await {
        Ok(false) => Ok(Some(path.clone())),
        Ok(true) => Ok(None),
        Err(error) => Err(io_error(path, &error)),
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
        feature = "unit",
        file = %file,
        removed,
        action = "roll_back",
        "pm3 backed out a file it had written before the plan failed",
    );
}

pub async fn linger_state(
    kind: UnitKind,
    programs: &UnitProgramSet,
    timeout_ms: u64,
) -> LingerState {
    match kind {
        UnitKind::Launchd | UnitKind::WinSchtasks => LingerState::Unknown,
        UnitKind::Systemd => query_linger(programs, timeout_ms).await,
    }
}

async fn query_linger(programs: &UnitProgramSet, timeout_ms: u64) -> LingerState {
    let Some(query) = loginctl_show_linger(programs) else {
        return LingerState::Unknown;
    };
    let Ok(captured) = capture(&query, timeout_ms).await else {
        return LingerState::Unknown;
    };
    parse_linger_state(captured.success, &captured.stdout)
}

pub async fn query_status(
    spec: &UnitSpec,
    programs: &UnitProgramSet,
    timeout_ms: u64,
) -> Result<UnitStatus, UnitCommandError> {
    if !spec.unit_path().is_file() {
        return Ok(UnitStatus::NotInstalled);
    }
    let captured = capture(&status_command(spec, programs), timeout_ms).await?;
    if parse_run_state(spec.kind, captured.success, &captured.stdout) {
        return Ok(UnitStatus::Running);
    }
    Ok(UnitStatus::InstalledNotRunning)
}

pub async fn query_supervised_pid(
    spec: &UnitSpec,
    programs: &UnitProgramSet,
    timeout_ms: u64,
) -> Result<Option<u32>, UnitCommandError> {
    let captured = capture(&supervised_pid_command(spec, programs), timeout_ms).await?;
    if !captured.success {
        return Ok(None);
    }
    Ok(match spec.kind {
        UnitKind::Launchd => parse_launchd_pid(&captured.stdout),
        UnitKind::Systemd => parse_main_pid(&captured.stdout),
        UnitKind::WinSchtasks => None,
    })
}

pub async fn hand_back_to_manager(
    spec: &UnitSpec,
    programs: &UnitProgramSet,
    timeout_ms: u64,
) -> Result<bool, UnitCommandError> {
    let Some(kickstart) = launchctl_kickstart(programs, &spec.label) else {
        return Ok(false);
    };
    run_command(&kickstart, timeout_ms).await?;
    Ok(true)
}

async fn run_step(step: &UnitStep, timeout_ms: u64) -> Result<Option<String>, UnitCommandError> {
    match step {
        UnitStep::Write {
            dir,
            path,
            contents,
        } => write_file(dir, path, contents).await.map(|()| None),
        UnitStep::Remove { path } => remove_path(path).await.map(|()| None),
        UnitStep::Run(command) => run_command(command, timeout_ms).await.map(|()| None),
        UnitStep::TryRun(command) => Ok(tolerate(command, timeout_ms).await),
    }
}

async fn tolerate(command: &UnitCommand, timeout_ms: u64) -> Option<String> {
    let Err(error) = run_command(command, timeout_ms).await else {
        return None;
    };
    let reason = error.to_string();
    let program = command.program.as_str();
    tracing::warn!(
        feature = "unit",
        program,
        reason,
        action = "skip_optional",
        "skipped an optional service manager command"
    );
    Some(reason)
}

async fn write_file(dir: &Path, path: &Path, contents: &str) -> Result<(), UnitCommandError> {
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(|error| io_error(dir, &error))?;
    crate::private_file::write_private(path, contents)
        .await
        .map_err(|error| io_error(path, &error))
}

async fn remove_path(path: &Path) -> Result<(), UnitCommandError> {
    tokio::fs::remove_file(path)
        .await
        .map_err(|error| io_error(path, &error))
}

async fn run_command(command: &UnitCommand, timeout_ms: u64) -> Result<(), UnitCommandError> {
    let captured = capture(command, timeout_ms).await?;
    if captured.success {
        return Ok(());
    }
    Err(UnitCommandError::Failed {
        program: command.program.clone(),
        reason: describe_refusal(&captured.stderr, captured.code),
    })
}

async fn capture(command: &UnitCommand, timeout_ms: u64) -> Result<Captured, UnitCommandError> {
    let started = Instant::now();
    let call = Command::new(&command.program)
        .args(&command.args)
        .envs(command.env.iter().map(|(name, value)| (name, value)))
        .output();
    let output = timeout(Duration::from_millis(timeout_ms), call)
        .await
        .map_err(|_elapsed| UnitCommandError::Stalled {
            program: command.program.clone(),
            timeout_ms,
        })?
        .map_err(|error| UnitCommandError::Spawn {
            program: command.program.clone(),
            reason: error.to_string(),
        })?;
    let captured = Captured::from_output(&output);
    let program = command.program.as_str();
    let code = captured.code;
    let duration_ms = elapsed_ms(started);
    tracing::debug!(
        feature = "unit",
        program,
        code,
        duration_ms,
        action = "service_command",
        "ran a service manager command"
    );
    Ok(captured)
}

fn elapsed_ms(started: Instant) -> u128 {
    started.elapsed().as_millis()
}

fn io_error(path: &Path, source: &std::io::Error) -> UnitCommandError {
    UnitCommandError::Io {
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
#[path = "../tests/unit_runner_tests.rs"]
mod tests;
