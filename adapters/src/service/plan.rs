use std::path::PathBuf;

use super::{
    command::{
        ServiceCommand, ServiceProgramSet, launchctl_list, launchctl_load, launchctl_unload,
        loginctl_enable_linger, systemctl_daemon_reload, systemctl_disable_now,
        systemctl_enable_now, systemctl_is_active,
    },
    launchd::render_plist,
    spec::{ServiceKind, ServiceUnitSpec},
    systemd::render_unit,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServiceStep {
    Write {
        dir: PathBuf,
        path: PathBuf,
        contents: String,
    },
    Remove {
        path: PathBuf,
    },
    Run(ServiceCommand),
}

#[must_use]
pub fn render_unit_contents(spec: &ServiceUnitSpec) -> String {
    match spec.kind {
        ServiceKind::Launchd => render_plist(spec),
        ServiceKind::Systemd => render_unit(spec),
    }
}

#[must_use]
pub fn install_plan(
    spec: &ServiceUnitSpec,
    programs: &ServiceProgramSet,
    config_contents: &str,
) -> Vec<ServiceStep> {
    let settle = ServiceStep::Write {
        dir: spec.working_directory.clone(),
        path: spec.config_path.clone(),
        contents: config_contents.to_string(),
    };
    let write = ServiceStep::Write {
        dir: spec.unit_dir.clone(),
        path: spec.unit_path(),
        contents: render_unit_contents(spec),
    };
    match spec.kind {
        ServiceKind::Launchd => vec![
            settle,
            write,
            ServiceStep::Run(launchctl_load(programs, &spec.unit_path())),
        ],
        ServiceKind::Systemd => vec![
            settle,
            write,
            ServiceStep::Run(systemctl_daemon_reload(programs)),
            ServiceStep::Run(systemctl_enable_now(programs, &spec.unit_name())),
            ServiceStep::Run(loginctl_enable_linger(programs)),
        ],
    }
}

#[must_use]
pub fn uninstall_plan(spec: &ServiceUnitSpec, programs: &ServiceProgramSet) -> Vec<ServiceStep> {
    let remove = ServiceStep::Remove {
        path: spec.unit_path(),
    };
    match spec.kind {
        ServiceKind::Launchd => vec![
            ServiceStep::Run(launchctl_unload(programs, &spec.unit_path())),
            remove,
        ],
        ServiceKind::Systemd => vec![
            ServiceStep::Run(systemctl_disable_now(programs, &spec.unit_name())),
            remove,
            ServiceStep::Run(systemctl_daemon_reload(programs)),
        ],
    }
}

#[must_use]
pub fn status_command(spec: &ServiceUnitSpec, programs: &ServiceProgramSet) -> ServiceCommand {
    match spec.kind {
        ServiceKind::Launchd => launchctl_list(programs, &spec.label),
        ServiceKind::Systemd => systemctl_is_active(programs, &spec.unit_name()),
    }
}

#[cfg(test)]
#[path = "../tests/service_plan_tests.rs"]
mod tests;
