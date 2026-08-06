use usecases::{CommandWrapper, SandboxError, SandboxPolicy, WrappedCommand};

use super::{
    backend::{HostSandbox, SandboxBackend},
    bwrap::bwrap_argv,
    seatbelt::seatbelt_argv,
};

#[derive(Clone, Debug)]
pub struct SandboxCommandWrapper {
    host: Option<HostSandbox>,
    minimal_read_roots: Vec<String>,
}

impl SandboxCommandWrapper {
    #[must_use]
    pub const fn new(host: Option<HostSandbox>, minimal_read_roots: Vec<String>) -> Self {
        Self {
            host,
            minimal_read_roots,
        }
    }

    #[must_use]
    pub fn backend(&self) -> Option<SandboxBackend> {
        self.host.as_ref().map(|host| host.backend)
    }
}

impl CommandWrapper for SandboxCommandWrapper {
    fn wrap(
        &self,
        app: &str,
        policy: &SandboxPolicy,
        program: &str,
        args: &[String],
    ) -> Result<WrappedCommand, SandboxError> {
        if policy.mode.is_unconfined() {
            return Ok(WrappedCommand {
                program: program.to_string(),
                args: args.to_vec(),
            });
        }
        let Some(host) = self.host.as_ref() else {
            return Err(SandboxError::NoBackend {
                app: app.to_string(),
            });
        };
        Ok(match host.backend {
            SandboxBackend::Seatbelt => seatbelt_argv(&host.program, policy, program, args),
            SandboxBackend::Bwrap => bwrap_argv(
                &host.program,
                &self.minimal_read_roots,
                policy,
                program,
                args,
            ),
        })
    }
}

#[cfg(test)]
#[path = "../tests/sandbox_wrapper_tests.rs"]
mod tests;
