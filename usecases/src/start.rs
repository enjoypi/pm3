use entities::{AppSpec, DependencyNode, topo_sort, validate_spec};

use crate::{
    Ports, Result, UsecaseError, log_paths::log_paths, persist::save_table, ports::LaunchSpec,
    table::ProcessTable,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StartOutcome {
    pub pm_id: u32,
    pub name: String,
    pub pid: Option<u32>,
    pub already_running: bool,
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
    let nodes: Vec<DependencyNode<'_>> = specs.iter().map(AppSpec::dependency_node).collect();
    let order = topo_sort(&nodes)?;

    let now_ms = ports.now_ms();
    for spec in specs {
        table.upsert(spec.clone(), now_ms);
    }

    let mut outcomes = Vec::with_capacity(order.len());
    for name in &order {
        outcomes.push(start_one(table, name, logs_dir, ports).await?);
    }
    save_table(table, ports).await?;
    Ok(outcomes)
}

pub(crate) async fn start_one(
    table: &mut ProcessTable,
    name: &str,
    logs_dir: &str,
    ports: &impl Ports,
) -> Result<StartOutcome> {
    let Some(record) = table.find_by_name_mut(name) else {
        return Err(UsecaseError::NotFound(name.to_string()));
    };

    if record.runtime.status.is_running() {
        return Ok(StartOutcome {
            pm_id: record.runtime.pm_id,
            name: name.to_string(),
            pid: record.runtime.pid,
            already_running: true,
        });
    }

    let launch = build_launch_spec(&record.spec, logs_dir, ports)?;
    let launched = ports.spawn(&launch).await?;
    let now_ms = ports.now_ms();

    record.runtime.mark_launched(launched.pid, now_ms);
    record.runtime.mark_online();
    Ok(StartOutcome {
        pm_id: record.runtime.pm_id,
        name: name.to_string(),
        pid: Some(launched.pid),
        already_running: false,
    })
}

fn build_launch_spec(spec: &AppSpec, logs_dir: &str, ports: &impl Ports) -> Result<LaunchSpec> {
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
