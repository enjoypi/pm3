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
    save_table(table, ports).await?;
    Ok(action)
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

    let last_uptime_ms = record.runtime.uptime_ms(now_ms).unwrap_or(0);
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

    let decision = decide_restart(
        record.spec.restart_policy(),
        last_uptime_ms,
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
    if outcome.clean() {
        ProcessStatus::Stopped
    } else {
        ProcessStatus::Errored
    }
}

#[cfg(test)]
#[path = "tests/supervise_tests.rs"]
mod tests;
