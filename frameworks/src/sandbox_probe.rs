use adapters::{SandboxBackend, program_available};

#[cfg(target_os = "macos")]
const PREFERRED_BACKENDS: [SandboxBackend; 1] = [SandboxBackend::Seatbelt];
#[cfg(not(target_os = "macos"))]
const PREFERRED_BACKENDS: [SandboxBackend; 1] = [SandboxBackend::Bwrap];

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

#[cfg(test)]
#[path = "tests/sandbox_probe_tests.rs"]
mod tests;
