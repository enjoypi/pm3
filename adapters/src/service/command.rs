use std::path::Path;

pub const LAUNCHCTL_PROGRAM: &str = "/bin/launchctl";
pub const SYSTEMCTL_PROGRAM: &str = "/usr/bin/systemctl";
pub const LOGINCTL_PROGRAM: &str = "/usr/bin/loginctl";

const USER_SCOPE: &str = "--user";
const OVERRIDE_DISABLED: &str = "-w";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceCommand {
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceProgramSet {
    pub launchctl: String,
    pub systemctl: String,
    pub loginctl: String,
}

impl Default for ServiceProgramSet {
    fn default() -> Self {
        Self {
            launchctl: LAUNCHCTL_PROGRAM.to_string(),
            systemctl: SYSTEMCTL_PROGRAM.to_string(),
            loginctl: LOGINCTL_PROGRAM.to_string(),
        }
    }
}

#[must_use]
pub fn launchctl_load(programs: &ServiceProgramSet, unit_path: &Path) -> ServiceCommand {
    command(
        &programs.launchctl,
        &["load", OVERRIDE_DISABLED, &unit_path.to_string_lossy()],
    )
}

#[must_use]
pub fn launchctl_unload(programs: &ServiceProgramSet, unit_path: &Path) -> ServiceCommand {
    command(
        &programs.launchctl,
        &["unload", OVERRIDE_DISABLED, &unit_path.to_string_lossy()],
    )
}

#[must_use]
pub fn launchctl_list(programs: &ServiceProgramSet, label: &str) -> ServiceCommand {
    command(&programs.launchctl, &["list", label])
}

#[must_use]
pub fn systemctl_daemon_reload(programs: &ServiceProgramSet) -> ServiceCommand {
    command(&programs.systemctl, &[USER_SCOPE, "daemon-reload"])
}

#[must_use]
pub fn systemctl_enable_now(programs: &ServiceProgramSet, unit_name: &str) -> ServiceCommand {
    command(
        &programs.systemctl,
        &[USER_SCOPE, "enable", "--now", unit_name],
    )
}

#[must_use]
pub fn systemctl_disable_now(programs: &ServiceProgramSet, unit_name: &str) -> ServiceCommand {
    command(
        &programs.systemctl,
        &[USER_SCOPE, "disable", "--now", unit_name],
    )
}

#[must_use]
pub fn systemctl_is_active(programs: &ServiceProgramSet, unit_name: &str) -> ServiceCommand {
    command(&programs.systemctl, &[USER_SCOPE, "is-active", unit_name])
}

#[must_use]
pub fn loginctl_enable_linger(programs: &ServiceProgramSet) -> ServiceCommand {
    command(&programs.loginctl, &["enable-linger"])
}

fn command(program: &str, args: &[&str]) -> ServiceCommand {
    ServiceCommand {
        program: program.to_string(),
        args: args
            .iter()
            .map(|argument| (*argument).to_string())
            .collect(),
    }
}

#[cfg(test)]
#[path = "../tests/service_command_tests.rs"]
mod tests;
