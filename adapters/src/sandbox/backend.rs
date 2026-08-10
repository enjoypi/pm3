use crate::{config::SandboxConfig, program::resolve_executable};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxProgramSet {
    pub seatbelt: String,
    pub bwrap: String,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SandboxBackend {
    Seatbelt,
    Bwrap,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostSandbox {
    pub backend: SandboxBackend,
    pub program: String,
}

impl SandboxProgramSet {
    #[must_use]
    pub fn from_config(sandbox: &SandboxConfig) -> Self {
        Self {
            seatbelt: sandbox.seatbelt_program.clone(),
            bwrap: sandbox.bwrap_program.clone(),
        }
    }

    #[must_use]
    pub fn program(&self, backend: SandboxBackend) -> &str {
        match backend {
            SandboxBackend::Seatbelt => &self.seatbelt,
            SandboxBackend::Bwrap => &self.bwrap,
        }
    }
}

impl SandboxBackend {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Seatbelt => "seatbelt",
            Self::Bwrap => "bwrap",
        }
    }

    #[must_use]
    pub fn resolve(
        self,
        programs: &SandboxProgramSet,
        search_path: Option<&str>,
    ) -> Option<HostSandbox> {
        resolve_executable(programs.program(self), search_path).map(|program| HostSandbox {
            backend: self,
            program: program.to_string_lossy().into_owned(),
        })
    }
}

#[cfg(test)]
#[path = "../tests/sandbox_backend_tests.rs"]
mod tests;
