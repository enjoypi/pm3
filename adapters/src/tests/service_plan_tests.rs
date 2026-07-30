use std::path::Path;

use super::*;
use crate::service_specs::{program_set, spec_for};

const FAKE: &str = "/tmp/pm3-fake-manager";

fn plan_for(kind: ServiceKind, install: bool) -> Vec<ServiceStep> {
    let spec = spec_for(kind, Path::new("/home/dev"));
    let programs = program_set(FAKE);
    if install {
        return install_plan(&spec, &programs);
    }
    uninstall_plan(&spec, &programs)
}

fn described(steps: &[ServiceStep]) -> Vec<String> {
    steps
        .iter()
        .map(|step| match step {
            ServiceStep::Write {
                dir: _,
                path,
                contents: _,
            } => format!("write {}", path.display()),
            ServiceStep::Remove { path } => format!("remove {}", path.display()),
            ServiceStep::Run(command) => format!("run {}", command.args.join(" ")),
        })
        .collect()
}

#[test]
fn a_launchd_install_writes_the_plist_before_loading_it() {
    let steps = plan_for(ServiceKind::Launchd, true);
    assert_eq!(
        described(&steps),
        [
            "write /home/dev/Library/LaunchAgents/pm3-test.plist",
            "run load -w /home/dev/Library/LaunchAgents/pm3-test.plist",
        ]
    );
}

#[test]
fn a_systemd_install_reloads_enables_and_lingers() {
    let steps = plan_for(ServiceKind::Systemd, true);
    assert_eq!(
        described(&steps),
        [
            "write /home/dev/.config/systemd/user/pm3-test.service",
            "run --user daemon-reload",
            "run --user enable --now pm3-test.service",
            "run enable-linger",
        ]
    );
}

#[test]
fn a_launchd_uninstall_unloads_before_removing_the_plist() {
    let steps = plan_for(ServiceKind::Launchd, false);
    assert_eq!(
        described(&steps),
        [
            "run unload -w /home/dev/Library/LaunchAgents/pm3-test.plist",
            "remove /home/dev/Library/LaunchAgents/pm3-test.plist",
        ]
    );
}

#[test]
fn a_systemd_uninstall_disables_removes_then_reloads() {
    let steps = plan_for(ServiceKind::Systemd, false);
    assert_eq!(
        described(&steps),
        [
            "run --user disable --now pm3-test.service",
            "remove /home/dev/.config/systemd/user/pm3-test.service",
            "run --user daemon-reload",
        ]
    );
}

#[test]
fn a_launchd_install_carries_the_rendered_plist() {
    let steps = plan_for(ServiceKind::Launchd, true);
    assert!(
        contents_of(&steps).contains("<key>RunAtLoad</key>"),
        "got: {}",
        contents_of(&steps)
    );
}

#[test]
fn a_systemd_install_carries_the_rendered_unit() {
    let steps = plan_for(ServiceKind::Systemd, true);
    assert!(
        contents_of(&steps).contains("WantedBy=default.target"),
        "got: {}",
        contents_of(&steps)
    );
}

#[test]
fn launchd_status_asks_for_the_agent_listing() {
    let spec = spec_for(ServiceKind::Launchd, Path::new("/home/dev"));
    let command = status_command(&spec, &program_set(FAKE));
    assert_eq!(command.args, ["list", "pm3-test"]);
}

#[test]
fn systemd_status_asks_whether_the_unit_is_active() {
    let spec = spec_for(ServiceKind::Systemd, Path::new("/home/dev"));
    let command = status_command(&spec, &program_set(FAKE));
    assert_eq!(command.args, ["--user", "is-active", "pm3-test.service"]);
}

fn contents_of(steps: &[ServiceStep]) -> String {
    for step in steps {
        if let ServiceStep::Write {
            dir: _,
            path: _,
            contents,
        } = step
        {
            return contents.clone();
        }
    }
    panic!("an install plan should write a unit file")
}
