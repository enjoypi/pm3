pub const SEATBELT_PROGRAM: &str = "/usr/bin/sandbox-exec";
pub const BWRAP_PROGRAM: &str = "bwrap";

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SandboxBackend {
    Seatbelt,
    Bwrap,
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
}

#[cfg(test)]
#[path = "../tests/sandbox_backend_tests.rs"]
mod tests;
