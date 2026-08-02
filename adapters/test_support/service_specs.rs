use std::{
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use crate::{ServiceKind, ServiceProgramSet, ServiceUnitSpec, service::unit_dir_of};

pub const LABEL: &str = "pm3-test";
pub const PROGRAM: &str = "/usr/local/bin/pm3";
pub const SEARCH_PATH: &str = "/usr/bin:/bin";
pub const MISSING_PROGRAM: &str = "/nonexistent/pm3-service-manager";
pub const RESTART_CONDITION: &str = "always";

const ROOT_DIR: &str = ".pm3";
const CONFIG_FILE: &str = "config.yaml";
const LOG_FILE: &str = "pm3.log";

pub fn spec_for(kind: ServiceKind, home: &Path) -> ServiceUnitSpec {
    let root = home.join(ROOT_DIR);
    ServiceUnitSpec {
        kind,
        label: LABEL.to_string(),
        unit_dir: unit_dir_of(kind, home),
        program: PathBuf::from(PROGRAM),
        config_path: root.join(CONFIG_FILE),
        working_directory: root.clone(),
        log_path: root.join(LOG_FILE),
        search_path: SEARCH_PATH.to_string(),
        home: home.to_string_lossy().into_owned(),
        restart_delay_secs: 2,
        restart_condition: RESTART_CONDITION.to_string(),
    }
}

pub fn program_set(program: &str) -> ServiceProgramSet {
    ServiceProgramSet {
        launchctl: program.to_string(),
        systemctl: program.to_string(),
        loginctl: program.to_string(),
    }
}

pub fn fake_program(dir: &Path, name: &str, script: &str) -> String {
    let path = dir.join(name);
    let body = format!("#!/bin/sh\n{script}\n");
    std::fs::write(&path, body).expect("internal error: the fake program directory is writable");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
        .expect("internal error: the fake program was just created");
    path.to_string_lossy().into_owned()
}
