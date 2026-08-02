use std::path::PathBuf;

use super::{
    command::{
        UnitCommand, UnitProgramSet, launchctl_list, launchctl_load, launchctl_unload,
        loginctl_enable_linger, systemctl_daemon_reload, systemctl_disable_now,
        systemctl_enable_now, systemctl_is_active,
    },
    launchd::render_plist,
    spec::{UnitKind, UnitSpec},
    systemd::render_unit,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UnitStep {
    Write {
        dir: PathBuf,
        path: PathBuf,
        contents: String,
    },
    Remove {
        path: PathBuf,
    },
    Run(UnitCommand),
    TryRun(UnitCommand),
}

#[must_use]
pub fn render_unit_contents(spec: &UnitSpec) -> String {
    match spec.kind {
        UnitKind::Launchd => render_plist(spec),
        UnitKind::Systemd => render_unit(spec),
    }
}

#[must_use]
pub fn install_plan(
    spec: &UnitSpec,
    programs: &UnitProgramSet,
    config_contents: &str,
) -> Vec<UnitStep> {
    let settle = UnitStep::Write {
        dir: spec.working_directory.clone(),
        path: spec.config_path.clone(),
        contents: config_contents.to_string(),
    };
    let write = UnitStep::Write {
        dir: spec.unit_dir.clone(),
        path: spec.unit_path(),
        contents: render_unit_contents(spec),
    };
    match spec.kind {
        UnitKind::Launchd => vec![
            settle,
            write,
            UnitStep::Run(launchctl_load(programs, &spec.unit_path())),
        ],
        UnitKind::Systemd => vec![
            settle,
            write,
            UnitStep::Run(systemctl_daemon_reload(programs)),
            UnitStep::Run(systemctl_enable_now(programs, &spec.unit_name())),
            UnitStep::TryRun(loginctl_enable_linger(programs)),
        ],
    }
}

#[must_use]
pub fn uninstall_plan(spec: &UnitSpec, programs: &UnitProgramSet) -> Vec<UnitStep> {
    let remove = UnitStep::Remove {
        path: spec.unit_path(),
    };
    match spec.kind {
        UnitKind::Launchd => vec![
            UnitStep::TryRun(launchctl_unload(programs, &spec.unit_path())),
            remove,
        ],
        UnitKind::Systemd => vec![
            UnitStep::TryRun(systemctl_disable_now(programs, &spec.unit_name())),
            remove,
            UnitStep::TryRun(systemctl_daemon_reload(programs)),
        ],
    }
}

#[must_use]
pub fn status_command(spec: &UnitSpec, programs: &UnitProgramSet) -> UnitCommand {
    match spec.kind {
        UnitKind::Launchd => launchctl_list(programs, &spec.label),
        UnitKind::Systemd => systemctl_is_active(programs, &spec.unit_name()),
    }
}

#[cfg(test)]
#[path = "../tests/unit_plan_tests.rs"]
mod tests;
