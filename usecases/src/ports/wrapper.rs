use entities::SandboxPolicy;
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WrappedCommand {
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Debug, Eq, PartialEq, Error)]
pub enum SandboxError {
    #[error(
        "cannot confine app '{app}': no usable sandbox backend on this platform, install bubblewrap or set sandbox mode to danger-full-access"
    )]
    NoBackend { app: String },
}

pub trait CommandWrapper: Send + Sync {
    fn wrap(
        &self,
        app: &str,
        policy: &SandboxPolicy,
        program: &str,
        args: &[String],
    ) -> Result<WrappedCommand, SandboxError>;
}

#[cfg(test)]
#[path = "../tests/ports_wrapper_tests.rs"]
mod tests;
