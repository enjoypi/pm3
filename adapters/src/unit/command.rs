use std::path::Path;

const USER_SCOPE: &str = "--user";
const OVERRIDE_DISABLED: &str = "-w";
const RUNTIME_DIR_VARIABLE: &str = "XDG_RUNTIME_DIR";
const SHOW_USER: &str = "show-user";
const PROPERTY_FLAG: &str = "-p";
const VALUE_FLAG: &str = "--value";
const LINGER_PROPERTY: &str = "Linger";
const MAIN_PID_PROPERTY: &str = "MainPID";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnitCommand {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnitProgramSet {
    pub launchctl: String,
    pub systemctl: String,
    pub loginctl: String,
    pub runtime_dir: Option<String>,
    pub uid: Option<u32>,
}

impl UnitProgramSet {
    #[must_use]
    pub fn from_config(
        service: &crate::config::ServiceConfig,
        runtime_dir: Option<&str>,
        uid: Option<u32>,
    ) -> Self {
        Self {
            launchctl: service.launchctl_path.clone(),
            systemctl: service.systemctl_path.clone(),
            loginctl: service.loginctl_path.clone(),
            runtime_dir: runtime_dir.map(ToString::to_string),
            uid,
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
    user_scoped(programs, &["daemon-reload"])
}

#[must_use]
pub fn systemctl_enable_now(programs: &UnitProgramSet, unit_name: &str) -> UnitCommand {
    user_scoped(programs, &["enable", "--now", unit_name])
}

#[must_use]
pub fn systemctl_disable_now(programs: &UnitProgramSet, unit_name: &str) -> UnitCommand {
    user_scoped(programs, &["disable", "--now", unit_name])
}

#[must_use]
pub fn systemctl_is_active(programs: &UnitProgramSet, unit_name: &str) -> UnitCommand {
    user_scoped(programs, &["is-active", unit_name])
}

#[must_use]
pub fn systemctl_show_main_pid(programs: &UnitProgramSet, unit_name: &str) -> UnitCommand {
    user_scoped(
        programs,
        &[
            "show",
            PROPERTY_FLAG,
            MAIN_PID_PROPERTY,
            VALUE_FLAG,
            unit_name,
        ],
    )
}

#[must_use]
pub fn launchctl_kickstart(programs: &UnitProgramSet, label: &str) -> Option<UnitCommand> {
    let uid = programs.uid?;
    let target = format!("gui/{uid}/{label}");
    Some(command(&programs.launchctl, &["kickstart", &target]))
}

#[must_use]
pub fn loginctl_enable_linger(programs: &UnitProgramSet) -> UnitCommand {
    command(&programs.loginctl, &["enable-linger"])
}

#[must_use]
pub fn loginctl_show_linger(programs: &UnitProgramSet) -> Option<UnitCommand> {
    let uid = programs.uid?;
    Some(command(
        &programs.loginctl,
        &[
            SHOW_USER,
            &uid.to_string(),
            PROPERTY_FLAG,
            LINGER_PROPERTY,
            VALUE_FLAG,
        ],
    ))
}

fn user_scoped(programs: &UnitProgramSet, args: &[&str]) -> UnitCommand {
    let scoped = [&[USER_SCOPE], args].concat();
    UnitCommand {
        env: runtime_environment(programs),
        ..command(&programs.systemctl, &scoped)
    }
}

fn runtime_environment(programs: &UnitProgramSet) -> Vec<(String, String)> {
    programs
        .runtime_dir
        .iter()
        .map(|dir| (RUNTIME_DIR_VARIABLE.to_string(), dir.clone()))
        .collect()
}

fn command(program: &str, args: &[&str]) -> UnitCommand {
    UnitCommand {
        program: program.to_string(),
        args: args
            .iter()
            .map(|argument| (*argument).to_string())
            .collect(),
        env: Vec::new(),
    }
}

#[cfg(test)]
#[path = "../tests/unit_command_tests.rs"]
mod tests;
