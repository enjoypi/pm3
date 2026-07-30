use usecases::{CommandWrapper, SandboxError, SandboxPolicy, WrappedCommand};

use super::{backend::SandboxBackend, bwrap::bwrap_argv, seatbelt::seatbelt_argv};

const UNSAFE_PATH_CHARACTERS: [char; 3] = ['"', '\\', '\n'];

#[derive(Copy, Clone, Debug)]
pub struct SandboxCommandWrapper {
    backend: Option<SandboxBackend>,
}

impl SandboxCommandWrapper {
    #[must_use]
    pub const fn new(backend: Option<SandboxBackend>) -> Self {
        Self { backend }
    }

    #[must_use]
    pub const fn backend(&self) -> Option<SandboxBackend> {
        self.backend
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
        let Some(backend) = self.backend else {
            return Err(SandboxError::NoBackend {
                app: app.to_string(),
            });
        };
        reject_unquotable_roots(app, policy)?;
        Ok(match backend {
            SandboxBackend::Seatbelt => seatbelt_argv(policy, program, args),
            SandboxBackend::Bwrap => bwrap_argv(policy, program, args),
        })
    }
}

fn reject_unquotable_roots(app: &str, policy: &SandboxPolicy) -> Result<(), SandboxError> {
    let granted = policy.granted_roots();
    let offending = granted
        .iter()
        .find(|root| root.contains(UNSAFE_PATH_CHARACTERS));
    let Some(root) = offending else {
        return Ok(());
    };
    Err(SandboxError::Unsupported {
        app: app.to_string(),
        reason: format!(
            "writable root '{root}' contains a quote, backslash or newline that cannot be expressed in a sandbox profile"
        ),
    })
}

#[cfg(test)]
#[path = "../tests/sandbox_wrapper_tests.rs"]
mod tests;
