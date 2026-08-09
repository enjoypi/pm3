use std::path::PathBuf;

use super::{
    command::{
        UnitCommand, UnitProgramSet, launchctl_list, launchctl_load, launchctl_unload,
        loginctl_enable_linger, schtasks_create, schtasks_delete, schtasks_end, schtasks_query,
        schtasks_run, systemctl_daemon_reload, systemctl_disable_now, systemctl_enable_now,
        systemctl_is_active, systemctl_show_main_pid,
    },
    launchd::render_plist,
    schtasks::{render_task_xml, render_wrapper},
    spec::{LingerState, UnitKind, UnitSpec},
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
        UnitKind::WinSchtasks => render_task_xml(spec),
    }
}

#[must_use]
pub fn install_plan(
    spec: &UnitSpec,
    programs: &UnitProgramSet,
    config_contents: &str,
    linger: LingerState,
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
        UnitKind::Systemd => systemd_install_steps(spec, programs, settle, write, linger),
        UnitKind::WinSchtasks => vec![
            settle,
            write,
            UnitStep::Write {
                dir: spec.unit_dir.clone(),
                path: spec.wrapper_path(),
                contents: render_wrapper(spec),
            },
            UnitStep::Run(schtasks_create(programs, &spec.label, &spec.unit_path())),
            UnitStep::Run(schtasks_run(programs, &spec.label)),
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
        UnitKind::WinSchtasks => vec![
            UnitStep::TryRun(schtasks_end(programs, &spec.label)),
            UnitStep::TryRun(schtasks_delete(programs, &spec.label)),
            remove,
            UnitStep::Remove {
                path: spec.wrapper_path(),
            },
        ],
    }
}

fn systemd_install_steps(
    spec: &UnitSpec,
    programs: &UnitProgramSet,
    settle: UnitStep,
    write: UnitStep,
    linger: LingerState,
) -> Vec<UnitStep> {
    let activate = vec![
        settle,
        write,
        UnitStep::Run(systemctl_daemon_reload(programs)),
        UnitStep::Run(systemctl_enable_now(programs, &spec.unit_name())),
    ];
    match linger {
        LingerState::Enabled => activate,
        LingerState::Unknown => [
            activate,
            vec![UnitStep::TryRun(loginctl_enable_linger(programs))],
        ]
        .concat(),
    }
}

#[must_use]
pub fn status_command(spec: &UnitSpec, programs: &UnitProgramSet) -> UnitCommand {
    match spec.kind {
        UnitKind::Launchd => launchctl_list(programs, &spec.label),
        UnitKind::Systemd => systemctl_is_active(programs, &spec.unit_name()),
        UnitKind::WinSchtasks => schtasks_query(programs, &spec.label),
    }
}

#[must_use]
pub fn supervised_pid_command(spec: &UnitSpec, programs: &UnitProgramSet) -> UnitCommand {
    match spec.kind {
        UnitKind::Launchd => launchctl_list(programs, &spec.label),
        UnitKind::Systemd => systemctl_show_main_pid(programs, &spec.unit_name()),
        UnitKind::WinSchtasks => schtasks_query(programs, &spec.label),
    }
}

#[must_use]
pub fn write_targets(spec: &UnitSpec) -> Vec<PathBuf> {
    let mut targets = vec![spec.config_path.clone(), spec.unit_path()];
    if spec.kind == UnitKind::WinSchtasks {
        targets.push(spec.wrapper_path());
    }
    targets
}

#[cfg(test)]
#[path = "../tests/unit_plan_tests.rs"]
mod tests;
