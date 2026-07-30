use std::{
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use crate::{ServiceKind, ServiceProgramSet, ServiceUnitSpec, service::unit_dir_of};

pub const LABEL: &str = "pm3-test";
pub const PROGRAM: &str = "/usr/local/bin/pm3";
pub const CONFIG_PATH: &str = "/etc/pm3/config.yaml";
pub const WORKING_DIRECTORY: &str = "/home/dev/.pm3";
pub const LOG_PATH: &str = "/home/dev/.pm3/pm3.log";
pub const SEARCH_PATH: &str = "/usr/bin:/bin";
pub const MISSING_PROGRAM: &str = "/nonexistent/pm3-service-manager";

pub fn spec_for(kind: ServiceKind, home: &Path) -> ServiceUnitSpec {
    ServiceUnitSpec {
        kind,
        label: LABEL.to_string(),
        unit_dir: unit_dir_of(kind, home),
        program: PathBuf::from(PROGRAM),
        config_path: PathBuf::from(CONFIG_PATH),
        working_directory: PathBuf::from(WORKING_DIRECTORY),
        log_path: PathBuf::from(LOG_PATH),
        search_path: SEARCH_PATH.to_string(),
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
