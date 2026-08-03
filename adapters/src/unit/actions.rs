use super::{
    command::UnitProgramSet,
    plan::{UnitStep, install_plan, uninstall_plan},
    runner::{UnitCommandError, execute_plan, linger_state, query_status},
    spec::UnitSpec,
};

pub const NOTHING_INSTALLED: &str = "no pm3 service is installed";

pub async fn install_unit(
    spec: &UnitSpec,
    programs: &UnitProgramSet,
    config_contents: &str,
    dry_run: bool,
    timeout_ms: u64,
) -> Result<String, UnitCommandError> {
    let linger = linger_state(spec.kind, programs, timeout_ms).await;
    let steps = install_plan(spec, programs, config_contents, linger);
    if dry_run {
        return Ok(render_plan(&steps));
    }
    let skipped = execute_plan(&steps, timeout_ms).await?;
    Ok(format!(
        "installed {} as a {} service\n{}\n{}{}",
        spec.label,
        spec.kind.as_str(),
        spec.config_path.display(),
        spec.unit_path().display(),
        render_skipped(&skipped)
    ))
}

pub async fn uninstall_unit(
    spec: &UnitSpec,
    programs: &UnitProgramSet,
    dry_run: bool,
    timeout_ms: u64,
) -> Result<String, UnitCommandError> {
    let steps = uninstall_plan(spec, programs);
    if dry_run {
        return Ok(render_plan(&steps));
    }
    if !spec.unit_path().is_file() {
        return Ok(NOTHING_INSTALLED.to_string());
    }
    let skipped = execute_plan(&steps, timeout_ms).await?;
    Ok(format!(
        "uninstalled {}{}",
        spec.label,
        render_skipped(&skipped)
    ))
}

pub async fn status_report(
    spec: &UnitSpec,
    programs: &UnitProgramSet,
    timeout_ms: u64,
) -> Result<String, UnitCommandError> {
    let status = query_status(spec, programs, timeout_ms).await?;
    Ok(format!(
        "{} ({} service): {}\n{}",
        spec.label,
        spec.kind.as_str(),
        status.as_str(),
        spec.unit_path().display()
    ))
}

fn render_skipped(skipped: &[String]) -> String {
    if skipped.is_empty() {
        return String::new();
    }
    format!("\nskipped: {}", skipped.join("; "))
}

fn render_plan(steps: &[UnitStep]) -> String {
    steps.iter().map(render_step).collect::<Vec<_>>().join("\n")
}

fn render_step(step: &UnitStep) -> String {
    match step {
        UnitStep::Write {
            dir: _,
            path,
            contents,
        } => format!("write {}\n{contents}", path.display()),
        UnitStep::Remove { path } => format!("remove {}", path.display()),
        UnitStep::Run(command) => {
            format!("run {} {}", command.program, command.args.join(" "))
        }
        UnitStep::TryRun(command) => {
            format!("try {} {}", command.program, command.args.join(" "))
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit_actions_tests.rs"]
mod tests;
