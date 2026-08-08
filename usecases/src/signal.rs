use entities::parse_signal_name;

use crate::{
    Liveness, Ports, Result, SignalScope, UsecaseError, selector::AppSelector, table::ProcessTable,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignalOutcome {
    pub name: String,
    pub signal: String,
}

pub async fn signal_app(
    table: &mut ProcessTable,
    selector: &AppSelector,
    raw_signal: &str,
    ports: &impl Ports,
) -> Result<SignalOutcome> {
    let signal = parse_signal_name(raw_signal)?;
    let record = table
        .find_mut(selector)
        .ok_or_else(|| UsecaseError::NotFound(selector.to_string()))?;
    let name = record.runtime.name.clone();
    let live = record
        .runtime
        .pid
        .filter(|_pid| !record.runtime.status.is_settled())
        .ok_or_else(|| UsecaseError::NotRunning(name.clone()))?;
    let token = record
        .runtime
        .identity
        .as_ref()
        .map(|identity| identity.token.clone());
    let observed = ports.identity(live).await;
    let owns_pid = matches!(observed, Liveness::Alive(ref current) if token.as_deref() == Some(current.as_str()));
    if !owns_pid {
        return Err(UsecaseError::NotRunning(name));
    }
    ports
        .deliver(&signal, live, SignalScope::ProcessGroup)
        .await?;
    log_signalled(&name, live, &signal);
    Ok(SignalOutcome { name, signal })
}

fn log_signalled(app: &str, pid: u32, signal: &str) {
    tracing::info!(
        feature = "lifecycle",
        action = "signal",
        app,
        pid,
        signal,
        "pm3 delivered a signal to a service",
    );
}

#[cfg(test)]
#[path = "tests/signal_tests.rs"]
mod tests;
