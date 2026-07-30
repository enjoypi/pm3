use std::path::Path;

use adapters::SandboxBackend;

#[cfg(target_os = "macos")]
const PREFERRED_BACKENDS: [SandboxBackend; 1] = [SandboxBackend::Seatbelt];
#[cfg(not(target_os = "macos"))]
const PREFERRED_BACKENDS: [SandboxBackend; 1] = [SandboxBackend::Bwrap];

const PATH_SEPARATOR: char = ':';

#[must_use]
pub fn detect_host_backend() -> Option<SandboxBackend> {
    let path_env = std::env::var("PATH").ok();
    probe_backend(&|program| program_available(program, path_env.as_deref()))
}

#[must_use]
pub fn probe_backend(available: &dyn Fn(&str) -> bool) -> Option<SandboxBackend> {
    PREFERRED_BACKENDS
        .into_iter()
        .find(|backend| available(backend.program()))
}

#[must_use]
pub fn program_available(program: &str, path_env: Option<&str>) -> bool {
    if program.starts_with('/') {
        return Path::new(program).is_file();
    }
    let Some(directories) = path_env else {
        return false;
    };
    directories
        .split(PATH_SEPARATOR)
        .any(|directory| Path::new(directory).join(program).is_file())
}

#[cfg(test)]
#[path = "tests/sandbox_probe_tests.rs"]
mod tests;
