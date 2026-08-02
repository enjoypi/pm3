use entities::{AppSpec, DependencyNode, ProcessIdentity, topo_sort, validate_spec};

use crate::{
    Ports, Result, UsecaseError, fingerprint::render_identity, log_paths::log_paths,
    persist::save_table, ports::LaunchSpec, selector::AppSelector, table::ProcessTable,
};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum StartMode {
    Register,
    Fire,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum StartKind {
    Spawned,
    AlreadyRunning,
    Adopted,
    Scheduled,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StartOutcome {
    pub pm_id: u32,
    pub name: String,
    pub pid: Option<u32>,
    pub kind: StartKind,
}

#[derive(Debug, Default)]
pub struct StartReport {
    pub outcomes: Vec<StartOutcome>,
    pub failure: Option<UsecaseError>,
}

impl StartReport {
    const fn refused(error: UsecaseError) -> Self {
        Self {
            outcomes: Vec::new(),
            failure: Some(error),
        }
    }
}

impl StartKind {
    #[must_use]
    pub const fn needs_watching(self) -> bool {
        matches!(self, Self::Spawned | Self::Adopted)
    }

    #[must_use]
    pub const fn needs_timer(self) -> bool {
        matches!(self, Self::Spawned | Self::Adopted | Self::Scheduled)
    }
}

pub async fn start_apps(
    table: &mut ProcessTable,
    specs: &[AppSpec],
    logs_dir: &str,
    ports: &impl Ports,
) -> StartReport {
    let order = match accepted_order(table, specs) {
        Ok(order) => order,
        Err(error) => return StartReport::refused(error),
    };

    let now_ms = ports.now_ms();
    let mut fresh = Vec::with_capacity(specs.len());
    for spec in specs {
        if table.find_by_name(&spec.name).is_none() {
            fresh.push(spec.name.clone());
        }
        table.upsert(spec.clone(), now_ms);
    }

    let mut report = StartReport {
        outcomes: Vec::with_capacity(order.len()),
        failure: None,
    };
    let mut failed_app = String::new();
    for name in &order {
        match launch(table, name, logs_dir, ports, StartMode::Register).await {
            Ok(outcome) => report.outcomes.push(outcome),
            Err(error) => {
                log_abandoned_start(name, &error);
                failed_app.clone_from(name);
                report.failure = Some(error);
                break;
            }
        }
    }
    if report.failure.is_some() {
        forget_unlaunched(table, &fresh, &report.outcomes);
    }
    if let Err(error) = save_table(table, ports).await {
        if report.failure.is_none() {
            report.failure = Some(error);
        } else {
            log_unsaved_table(&failed_app, &error);
        }
    }
    report
}

fn forget_unlaunched(table: &mut ProcessTable, fresh: &[String], outcomes: &[StartOutcome]) {
    for name in fresh {
        let launched = outcomes.iter().any(|outcome| outcome.name == *name);
        if !launched {
            table.remove(&AppSelector::Name(name.clone()));
        }
    }
}

fn log_unsaved_table(app: &str, error: &UsecaseError) {
    let reason = error.to_string();
    tracing::warn!(
        feature = "lifecycle",
        action = "start",
        app,
        reason,
        "pm3 cannot persist the process table after a failed start, so a daemon restart may lose the services it just started",
    );
}

fn accepted_order(table: &ProcessTable, specs: &[AppSpec]) -> Result<Vec<String>> {
    for spec in specs {
        validate_spec(spec)?;
    }
    start_order(table, specs)
}

fn log_abandoned_start(app: &str, error: &UsecaseError) {
    let reason = error.to_string();
    tracing::warn!(
        feature = "lifecycle",
        action = "start",
        app,
        reason,
        "pm3 cannot start a service, so it leaves the rest of the batch alone",
    );
}

fn start_order(table: &ProcessTable, specs: &[AppSpec]) -> Result<Vec<String>> {
    let known = table.dependency_nodes();
    let mut nodes: Vec<DependencyNode<'_>> = specs.iter().map(AppSpec::dependency_node).collect();
    nodes.extend(
        known
            .into_iter()
            .filter(|node| !names_include(specs, node.name)),
    );
    Ok(topo_sort(&nodes)?
        .into_iter()
        .filter(|name| names_include(specs, name))
        .collect())
}

fn names_include(specs: &[AppSpec], candidate: &str) -> bool {
    specs.iter().any(|spec| spec.name == candidate)
}

pub(crate) async fn start_one(
    table: &mut ProcessTable,
    name: &str,
    logs_dir: &str,
    ports: &impl Ports,
) -> Result<StartOutcome> {
    launch(table, name, logs_dir, ports, StartMode::Fire).await
}

async fn launch(
    table: &mut ProcessTable,
    name: &str,
    logs_dir: &str,
    ports: &impl Ports,
    mode: StartMode,
) -> Result<StartOutcome> {
    let Some(record) = table.find_by_name_mut(name) else {
        return Err(UsecaseError::NotFound(name.to_string()));
    };

    if !record.runtime.status.is_settled() {
        record.runtime.arm_schedule();
        if record.runtime.status.is_shutting_down() {
            record.runtime.request_restart();
        }
        return Ok(StartOutcome {
            pm_id: record.runtime.pm_id,
            name: name.to_string(),
            pid: record.runtime.pid,
            kind: StartKind::AlreadyRunning,
        });
    }

    if mode == StartMode::Register && record.spec.is_scheduled_task() {
        record.runtime.arm_schedule();
        return Ok(StartOutcome {
            pm_id: record.runtime.pm_id,
            name: name.to_string(),
            pid: None,
            kind: StartKind::Scheduled,
        });
    }

    let launch = build_launch_spec(&record.spec, logs_dir, ports)?;
    let launched = ports.spawn(&launch).await?;
    let now_ms = ports.now_ms();
    let identity = capture_identity(&record.spec, launched.pid, ports).await;

    record.runtime.arm_schedule();
    record.runtime.mark_launched(launched.pid, now_ms);
    record.runtime.mark_online();
    record.runtime.record_identity(identity);
    log_started(name, launched.pid);
    Ok(StartOutcome {
        pm_id: record.runtime.pm_id,
        name: name.to_string(),
        pid: Some(launched.pid),
        kind: StartKind::Spawned,
    })
}

fn log_started(app: &str, pid: u32) {
    tracing::info!(
        feature = "lifecycle",
        action = "start",
        app,
        pid,
        "pm3 started a service",
    );
}

pub(crate) async fn capture_identity(
    spec: &AppSpec,
    pid: u32,
    ports: &impl Ports,
) -> Option<ProcessIdentity> {
    let token = ports.identity(pid).await.into_token()?;
    let launch_digest = ports.digest(&render_identity(spec));
    let binary_digest = ports
        .file_digest(&spec.script)
        .await
        .inspect_err(|error| log_unusable_identity(&spec.name, &error.to_string()))
        .ok()?;
    Some(ProcessIdentity {
        token,
        launch_digest,
        binary_digest,
    })
}

fn log_unusable_identity(app: &str, reason: &str) {
    tracing::warn!(
        feature = "supervisor",
        action = "identity",
        app,
        reason,
        "pm3 cannot fingerprint a launched service, so a daemon restart will restart it",
    );
}

pub(crate) fn build_launch_spec(
    spec: &AppSpec,
    logs_dir: &str,
    ports: &impl Ports,
) -> Result<LaunchSpec> {
    let wrapped = ports.wrap(&spec.name, &spec.sandbox, &spec.script, &spec.args)?;
    let paths = log_paths(logs_dir, &spec.name);
    Ok(LaunchSpec {
        name: spec.name.clone(),
        program: wrapped.program,
        args: wrapped.args,
        cwd: spec.cwd.clone(),
        env: spec.env.clone(),
        stdout_path: paths.stdout,
        stderr_path: paths.stderr,
    })
}

#[cfg(test)]
#[path = "tests/start_tests.rs"]
mod tests;
