use std::path::Path;

use super::*;
use crate::unit_specs::{program_set, spec_for};

const FAKE: &str = "/tmp/pm3-fake-manager";
const CONFIG_BODY: &str = "pm3:\n  home: \"~/.pm3\"\n";

fn plan_for(kind: UnitKind, install: bool) -> Vec<UnitStep> {
    let spec = spec_for(kind, Path::new("/home/dev"));
    let programs = program_set(FAKE);
    if install {
        return install_plan(&spec, &programs, CONFIG_BODY, LingerState::Unknown);
    }
    uninstall_plan(&spec, &programs)
}

fn described(steps: &[UnitStep]) -> Vec<String> {
    steps
        .iter()
        .map(|step| match step {
            UnitStep::Write {
                dir: _,
                path,
                contents: _,
            } => format!("write {}", path.display()),
            UnitStep::Remove { path } => format!("remove {}", path.display()),
            UnitStep::Run(command) => format!("run {}", command.args.join(" ")),
            UnitStep::TryRun(command) => format!("try {}", command.args.join(" ")),
        })
        .collect()
}

#[test]
fn a_launchd_install_writes_the_plist_before_loading_it() {
    let steps = plan_for(UnitKind::Launchd, true);
    assert_eq!(
        described(&steps),
        [
            "write /home/dev/.pm3/config.yaml",
            "write /home/dev/Library/LaunchAgents/pm3-test.plist",
            "run load -w /home/dev/Library/LaunchAgents/pm3-test.plist",
        ]
    );
}

#[test]
fn a_systemd_install_reloads_enables_and_tries_to_linger() {
    let steps = plan_for(UnitKind::Systemd, true);
    assert_eq!(
        described(&steps),
        [
            "write /home/dev/.pm3/config.yaml",
            "write /home/dev/.config/systemd/user/pm3-test.service",
            "run --user daemon-reload",
            "run --user enable --now pm3-test.service",
            "try enable-linger",
        ]
    );
}

#[test]
fn a_systemd_install_asks_for_no_permission_once_the_user_lingers() {
    let spec = spec_for(UnitKind::Systemd, Path::new("/home/dev"));
    let steps = install_plan(&spec, &program_set(FAKE), CONFIG_BODY, LingerState::Enabled);
    assert_eq!(
        described(&steps),
        [
            "write /home/dev/.pm3/config.yaml",
            "write /home/dev/.config/systemd/user/pm3-test.service",
            "run --user daemon-reload",
            "run --user enable --now pm3-test.service",
        ],
        "enable-linger goes through polkit, so an already lingering user must not be asked"
    );
}

#[test]
fn a_launchd_uninstall_unloads_before_removing_the_plist() {
    let steps = plan_for(UnitKind::Launchd, false);
    assert_eq!(
        described(&steps),
        [
            "try unload -w /home/dev/Library/LaunchAgents/pm3-test.plist",
            "remove /home/dev/Library/LaunchAgents/pm3-test.plist",
        ],
        "a manager that refuses to unload must not strand the unit file"
    );
}

#[test]
fn a_systemd_uninstall_disables_removes_then_reloads() {
    let steps = plan_for(UnitKind::Systemd, false);
    assert_eq!(
        described(&steps),
        [
            "try --user disable --now pm3-test.service",
            "remove /home/dev/.config/systemd/user/pm3-test.service",
            "try --user daemon-reload",
        ],
        "a session without a bus must not strand the unit file"
    );
}

#[test]
fn a_launchd_install_carries_the_rendered_plist() {
    let steps = plan_for(UnitKind::Launchd, true);
    assert!(
        contents_of(&steps).contains("<key>RunAtLoad</key>"),
        "got: {}",
        contents_of(&steps)
    );
}

#[test]
fn a_systemd_install_carries_the_rendered_unit() {
    let steps = plan_for(UnitKind::Systemd, true);
    assert!(
        contents_of(&steps).contains("WantedBy=default.target"),
        "got: {}",
        contents_of(&steps)
    );
}

#[test]
fn an_install_settles_the_config_next_to_the_runtime_state() {
    let steps = plan_for(UnitKind::Launchd, true);
    let UnitStep::Write {
        dir,
        path: _,
        contents,
    } = &steps[0]
    else {
        panic!("an install plan should start by writing the config")
    };
    assert_eq!(dir, Path::new("/home/dev/.pm3"));
    assert_eq!(contents, CONFIG_BODY);
}

#[test]
fn launchd_status_asks_for_the_agent_listing() {
    let spec = spec_for(UnitKind::Launchd, Path::new("/home/dev"));
    let command = status_command(&spec, &program_set(FAKE));
    assert_eq!(command.args, ["list", "pm3-test"]);
}

#[test]
fn systemd_status_asks_whether_the_unit_is_active() {
    let spec = spec_for(UnitKind::Systemd, Path::new("/home/dev"));
    let command = status_command(&spec, &program_set(FAKE));
    assert_eq!(command.args, ["--user", "is-active", "pm3-test.service"]);
}

fn contents_of(steps: &[UnitStep]) -> String {
    for step in steps {
        if let UnitStep::Write {
            dir: _,
            path,
            contents,
        } = step
            && path.extension().is_some_and(|suffix| suffix != "yaml")
        {
            return contents.clone();
        }
    }
    panic!("an install plan should write a unit file")
}

#[test]
fn launchd_supervision_reads_the_pid_from_the_listing() {
    let spec = spec_for(UnitKind::Launchd, Path::new("/home/dev"));
    let command = supervised_pid_command(&spec, &program_set(FAKE));
    assert_eq!(command.args, ["list", "pm3-test"]);
}

#[test]
fn systemd_supervision_reads_the_main_pid_property() {
    let spec = spec_for(UnitKind::Systemd, Path::new("/home/dev"));
    let command = supervised_pid_command(&spec, &program_set(FAKE));
    assert_eq!(
        command.args,
        [
            "--user",
            "show",
            "-p",
            "MainPID",
            "--value",
            "pm3-test.service"
        ]
    );
}

#[test]
fn an_install_overwrites_exactly_the_config_and_the_unit() {
    let spec = spec_for(UnitKind::Systemd, Path::new("/home/dev"));
    let targets = write_targets(&spec);
    assert_eq!(targets, [spec.config_path.clone(), spec.unit_path()]);
}
