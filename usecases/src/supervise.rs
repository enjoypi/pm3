use entities::{ProcessStatus, RestartDecision, decide_restart};

use crate::{
    Ports, Result, UsecaseError, persist::save_table, ports::ExitOutcome, table::ProcessTable,
};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ExitAction {
    RestartAfter { delay_ms: u64 },
    Settled { status: ProcessStatus },
}

pub async fn handle_child_exit(
    table: &mut ProcessTable,
    name: &str,
    outcome: ExitOutcome,
    ports: &impl Ports,
) -> Result<ExitAction> {
    let now_ms = ports.now_ms();
    let action = classify_exit(table, name, outcome, now_ms)?;
    if let Err(error) = save_table(table, ports).await {
        log_unsaved_exit(name, &error);
    }
    Ok(action)
}

fn log_unsaved_exit(app: &str, error: &UsecaseError) {
    let reason = error.to_string();
    tracing::warn!(
        feature = "supervisor",
        action = "exit",
        app,
        reason,
        "pm3 cannot persist the process table after a child exited, so a daemon restart may misjudge this service",
    );
}

pub async fn settle_failed_probe(
    table: &mut ProcessTable,
    name: &str,
    ports: &impl Ports,
) -> Result<()> {
    let record = table
        .find_by_name_mut(name)
        .ok_or_else(|| UsecaseError::NotFound(name.to_string()))?;
    record.runtime.mark_exited(ProcessStatus::Errored);
    save_table(table, ports).await?;
    Ok(())
}

fn classify_exit(
    table: &mut ProcessTable,
    name: &str,
    outcome: ExitOutcome,
    now_ms: u64,
) -> Result<ExitAction> {
    let record = table
        .find_by_name_mut(name)
        .ok_or_else(|| UsecaseError::NotFound(name.to_string()))?;

    let was_stopping = record.runtime.status.is_shutting_down();
    let restart_requested = record.runtime.take_restart_request();

    if restart_requested {
        record.runtime.mark_exited(ProcessStatus::Stopped);
        return Ok(ExitAction::RestartAfter { delay_ms: 0 });
    }

    if was_stopping {
        record.runtime.mark_exited(ProcessStatus::Stopped);
        return Ok(ExitAction::Settled {
            status: ProcessStatus::Stopped,
        });
    }

    if let ExitOutcome::Code(code) = outcome
        && record.spec.stops_on(code)
    {
        record.runtime.mark_exited(ProcessStatus::Stopped);
        return Ok(ExitAction::Settled {
            status: ProcessStatus::Stopped,
        });
    }

    let decision = decide_restart(
        record.spec.restart_policy(),
        record.runtime.uptime_ms(now_ms),
        record.runtime.unstable_restarts,
    );
    match decision {
        RestartDecision::Restart {
            delay_ms,
            unstable_restarts,
        } => {
            record.runtime.mark_exited(ProcessStatus::Stopped);
            record.runtime.count_restart(unstable_restarts);
            Ok(ExitAction::RestartAfter { delay_ms })
        }
        RestartDecision::GiveUp { unstable_restarts } => {
            let status = settled_status(outcome);
            record.runtime.mark_exited(status);
            record.runtime.unstable_restarts = unstable_restarts;
            Ok(ExitAction::Settled { status })
        }
    }
}

const fn settled_status(outcome: ExitOutcome) -> ProcessStatus {
    if outcome.failed() {
        ProcessStatus::Errored
    } else {
        ProcessStatus::Stopped
    }
}

#[cfg(test)]
#[path = "tests/supervise_tests.rs"]
mod tests;
