use std::path::Path;

const USER_SCOPE: &str = "--user";
const OVERRIDE_DISABLED: &str = "-w";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnitCommand {
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnitProgramSet {
    pub launchctl: String,
    pub systemctl: String,
    pub loginctl: String,
}

impl UnitProgramSet {
    #[must_use]
    pub fn from_config(service: &crate::config::ServiceConfig) -> Self {
        Self {
            launchctl: service.launchctl_path.clone(),
            systemctl: service.systemctl_path.clone(),
            loginctl: service.loginctl_path.clone(),
        }
    }
}

#[must_use]
pub fn launchctl_load(programs: &UnitProgramSet, unit_path: &Path) -> UnitCommand {
    command(
        &programs.launchctl,
        &["load", OVERRIDE_DISABLED, &unit_path.to_string_lossy()],
    )
}

#[must_use]
pub fn launchctl_unload(programs: &UnitProgramSet, unit_path: &Path) -> UnitCommand {
    command(
        &programs.launchctl,
        &["unload", OVERRIDE_DISABLED, &unit_path.to_string_lossy()],
    )
}

#[must_use]
pub fn launchctl_list(programs: &UnitProgramSet, label: &str) -> UnitCommand {
    command(&programs.launchctl, &["list", label])
}

#[must_use]
pub fn systemctl_daemon_reload(programs: &UnitProgramSet) -> UnitCommand {
    command(&programs.systemctl, &[USER_SCOPE, "daemon-reload"])
}

#[must_use]
pub fn systemctl_enable_now(programs: &UnitProgramSet, unit_name: &str) -> UnitCommand {
    command(
        &programs.systemctl,
        &[USER_SCOPE, "enable", "--now", unit_name],
    )
}

#[must_use]
pub fn systemctl_disable_now(programs: &UnitProgramSet, unit_name: &str) -> UnitCommand {
    command(
        &programs.systemctl,
        &[USER_SCOPE, "disable", "--now", unit_name],
    )
}

#[must_use]
pub fn systemctl_is_active(programs: &UnitProgramSet, unit_name: &str) -> UnitCommand {
    command(&programs.systemctl, &[USER_SCOPE, "is-active", unit_name])
}

#[must_use]
pub fn loginctl_enable_linger(programs: &UnitProgramSet) -> UnitCommand {
    command(&programs.loginctl, &["enable-linger"])
}

fn command(program: &str, args: &[&str]) -> UnitCommand {
    UnitCommand {
        program: program.to_string(),
        args: args
            .iter()
            .map(|argument| (*argument).to_string())
            .collect(),
    }
}

#[cfg(test)]
#[path = "../tests/unit_command_tests.rs"]
mod tests;
