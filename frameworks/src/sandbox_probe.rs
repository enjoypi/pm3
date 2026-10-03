#[cfg(unix)]
use adapters::SandboxBackend;
use adapters::{HostSandbox, SandboxProgramSet};

#[cfg(target_os = "macos")]
const PREFERRED_BACKENDS: [SandboxBackend; 1] = [SandboxBackend::Seatbelt];
#[cfg(all(unix, not(target_os = "macos")))]
const PREFERRED_BACKENDS: [SandboxBackend; 1] = [SandboxBackend::Bwrap];

#[cfg(unix)]
#[must_use]
pub fn detect_host_backend(programs: &SandboxProgramSet, search_path: &str) -> Option<HostSandbox> {
    probe_backend(&|backend| backend.resolve(programs, Some(search_path)))
}

#[cfg(windows)]
#[must_use]
pub const fn detect_host_backend(
    _programs: &SandboxProgramSet,
    _search_path: &str,
) -> Option<HostSandbox> {
    None
}

#[cfg(unix)]
#[must_use]
pub fn probe_backend(
    resolve: &dyn Fn(SandboxBackend) -> Option<HostSandbox>,
) -> Option<HostSandbox> {
    PREFERRED_BACKENDS.into_iter().find_map(resolve)
}

#[cfg(test)]
#[path = "tests/sandbox_probe_tests.rs"]
mod tests;
