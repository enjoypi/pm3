use crate::program::resolve_program;

pub const SEATBELT_PROGRAM: &str = "/usr/bin/sandbox-exec";
pub const BWRAP_PROGRAM: &str = "bwrap";

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

impl SandboxBackend {
    #[must_use]
    pub const fn program(self) -> &'static str {
        match self {
            Self::Seatbelt => SEATBELT_PROGRAM,
            Self::Bwrap => BWRAP_PROGRAM,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Seatbelt => "seatbelt",
            Self::Bwrap => "bwrap",
        }
    }

    #[must_use]
    pub fn resolve(self, search_path: Option<&str>) -> Option<HostSandbox> {
        let program = resolve_program(self.program(), search_path)?;
        Some(HostSandbox {
            backend: self,
            program: program.to_string_lossy().into_owned(),
        })
    }
}

#[cfg(test)]
#[path = "../tests/sandbox_backend_tests.rs"]
mod tests;
