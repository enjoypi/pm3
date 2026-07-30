use super::{
    command::ServiceProgramSet,
    plan::{ServiceStep, install_plan, uninstall_plan},
    runner::{ServiceCommandError, execute_plan, query_status},
    spec::ServiceUnitSpec,
};

pub const NOTHING_INSTALLED: &str = "no pm3 service is installed";

pub async fn install_service(
    spec: &ServiceUnitSpec,
    programs: &ServiceProgramSet,
    dry_run: bool,
) -> Result<String, ServiceCommandError> {
    let steps = install_plan(spec, programs);
    if dry_run {
        return Ok(render_plan(&steps));
    }
    execute_plan(&steps).await?;
    Ok(format!(
        "installed {} as a {} service\n{}",
        spec.label,
        spec.kind.as_str(),
        spec.unit_path().display()
    ))
}

pub async fn uninstall_service(
    spec: &ServiceUnitSpec,
    programs: &ServiceProgramSet,
    dry_run: bool,
) -> Result<String, ServiceCommandError> {
    let steps = uninstall_plan(spec, programs);
    if dry_run {
        return Ok(render_plan(&steps));
    }
    if !spec.unit_path().is_file() {
        return Ok(NOTHING_INSTALLED.to_string());
    }
    execute_plan(&steps).await?;
    Ok(format!("uninstalled {}", spec.label))
}

pub async fn status_report(
    spec: &ServiceUnitSpec,
    programs: &ServiceProgramSet,
) -> Result<String, ServiceCommandError> {
    let status = query_status(spec, programs).await?;
    Ok(format!(
        "{} ({} service): {}\n{}",
        spec.label,
        spec.kind.as_str(),
        status.as_str(),
        spec.unit_path().display()
    ))
}

fn render_plan(steps: &[ServiceStep]) -> String {
    steps.iter().map(render_step).collect::<Vec<_>>().join("\n")
}

fn render_step(step: &ServiceStep) -> String {
    match step {
        ServiceStep::Write {
            dir: _,
            path,
            contents,
        } => format!("write {}\n{contents}", path.display()),
        ServiceStep::Remove { path } => format!("remove {}", path.display()),
        ServiceStep::Run(command) => {
            format!("run {} {}", command.program, command.args.join(" "))
        }
    }
}

#[cfg(test)]
#[path = "../tests/service_actions_tests.rs"]
mod tests;
