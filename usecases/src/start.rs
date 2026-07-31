use entities::{AppSpec, DependencyNode, ProcessIdentity, topo_sort, validate_spec};

use crate::{
    Ports, Result, UsecaseError, fingerprint::render_identity, log_paths::log_paths,
    persist::save_table, ports::LaunchSpec, table::ProcessTable,
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
) -> Result<Vec<StartOutcome>> {
    for spec in specs {
        validate_spec(spec)?;
    }
    let order = start_order(table, specs)?;

    let now_ms = ports.now_ms();
    for spec in specs {
        table.upsert(spec.clone(), now_ms);
    }

    let mut outcomes = Vec::with_capacity(order.len());
    for name in &order {
        outcomes.push(launch(table, name, logs_dir, ports, StartMode::Register).await?);
    }
    save_table(table, ports).await?;
    Ok(outcomes)
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
        return Ok(StartOutcome {
            pm_id: record.runtime.pm_id,
            name: name.to_string(),
            pid: record.runtime.pid,
            kind: StartKind::AlreadyRunning,
        });
    }

    if mode == StartMode::Register && record.spec.is_scheduled_task() {
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

    record.runtime.mark_launched(launched.pid, now_ms);
    record.runtime.mark_online();
    record.runtime.record_identity(identity);
    Ok(StartOutcome {
        pm_id: record.runtime.pm_id,
        name: name.to_string(),
        pid: Some(launched.pid),
        kind: StartKind::Spawned,
    })
}

pub(crate) async fn capture_identity(
    spec: &AppSpec,
    pid: u32,
    ports: &impl Ports,
) -> Option<ProcessIdentity> {
    let token = ports.identity(pid).await?;
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
