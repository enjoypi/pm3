use std::path::{Path, PathBuf};

#[cfg(unix)]
pub const PATH_SEPARATOR: char = ':';
#[cfg(windows)]
pub const PATH_SEPARATOR: char = ';';
pub const HOME_PLACEHOLDER: &str = "${HOME}";
pub const SERVICE_CWD_NAME: &str = "PM3_SERVICE_CWD";
pub const SERVICE_CWD_PLACEHOLDER: &str = "${PM3_SERVICE_CWD}";

#[must_use]
pub fn program_available(program: &str, path_env: Option<&str>) -> bool {
    resolve_program(program, path_env).is_some()
}

#[must_use]
pub fn resolve_program(program: &str, path_env: Option<&str>) -> Option<PathBuf> {
    resolve_with(program, path_env, Path::is_file)
}

#[must_use]
pub fn resolve_executable(program: &str, path_env: Option<&str>) -> Option<PathBuf> {
    resolve_with(program, path_env, is_executable)
}

fn resolve_with(
    program: &str,
    path_env: Option<&str>,
    accepts: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    if program.starts_with('/') {
        return accepts(Path::new(program)).then(|| PathBuf::from(program));
    }
    let directories = path_env?;
    directories
        .split(PATH_SEPARATOR)
        .map(|directory| Path::new(directory).join(program))
        .find(|candidate| accepts(candidate))
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
    let Some(relative) = suffix.strip_prefix(['/', '\\']) else {
        return value.to_string();
    };
    format!("{HOME_PLACEHOLDER}/{relative}")
}

#[must_use]
pub fn fold_service_cwd(value: &str) -> String {
    let mut folded = String::with_capacity(value.len());
    let mut rest = value;
    while let Some((head, tail)) = rest.split_once(SERVICE_CWD_NAME) {
        folded.push_str(head);
        if head.ends_with("${") && tail.starts_with('}') {
            folded.push_str(SERVICE_CWD_NAME);
        } else {
            folded.push_str(SERVICE_CWD_PLACEHOLDER);
        }
        rest = tail;
    }
    folded.push_str(rest);
    folded
}

#[cfg(unix)]
fn is_executable(candidate: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    const EXECUTE_MASK: u32 = 0o111;
    candidate.is_file()
        && candidate
            .metadata()
            .is_ok_and(|meta| meta.permissions().mode() & EXECUTE_MASK != 0)
}

#[cfg(not(unix))]
fn is_executable(candidate: &Path) -> bool {
    candidate.is_file()
}

#[cfg(test)]
#[path = "tests/program_tests.rs"]
mod tests;
