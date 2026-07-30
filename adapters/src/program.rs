use std::path::{Path, PathBuf};

pub const PATH_SEPARATOR: char = ':';
pub const HOME_PLACEHOLDER: &str = "${HOME}";
pub const SVC_CWD_NAME: &str = "PM3_SVC_CWD";
pub const SVC_CWD_PLACEHOLDER: &str = "${PM3_SVC_CWD}";

#[must_use]
pub fn program_available(program: &str, path_env: Option<&str>) -> bool {
    resolve_program(program, path_env).is_some()
}

#[must_use]
pub fn resolve_program(program: &str, path_env: Option<&str>) -> Option<PathBuf> {
    if program.starts_with('/') {
        return Path::new(program).is_file().then(|| PathBuf::from(program));
    }
    let directories = path_env?;
    directories
        .split(PATH_SEPARATOR)
        .map(|directory| Path::new(directory).join(program))
        .find(|candidate| candidate.is_file())
}

#[must_use]
pub fn fold_home(value: &str, home: Option<&str>) -> String {
    let Some(home) = home.filter(|candidate| !candidate.is_empty()) else {
        return value.to_string();
    };
    let Some(suffix) = value.strip_prefix(home) else {
        return value.to_string();
    };
    if suffix.is_empty() {
        return HOME_PLACEHOLDER.to_string();
    }
    let Some(relative) = suffix.strip_prefix('/') else {
        return value.to_string();
    };
    format!("{HOME_PLACEHOLDER}/{relative}")
}

#[must_use]
pub fn fold_svc_cwd(value: &str) -> String {
    if value.contains(SVC_CWD_PLACEHOLDER) {
        return value.to_string();
    }
    value.replace(SVC_CWD_NAME, SVC_CWD_PLACEHOLDER)
}

#[cfg(test)]
#[path = "tests/program_tests.rs"]
mod tests;
