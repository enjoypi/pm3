use usecases::{CommandWrapper, SandboxError, SandboxPolicy, WrappedCommand};

use super::{
    backend::{HostSandbox, SandboxBackend},
    bwrap::bwrap_argv,
    seatbelt::seatbelt_argv,
};

const UNRENDERABLE_PATH_CHARACTERS: [char; 1] = ['\n'];

#[derive(Clone, Debug)]
pub struct SandboxCommandWrapper {
    host: Option<HostSandbox>,
}

impl SandboxCommandWrapper {
    #[must_use]
    pub const fn new(host: Option<HostSandbox>) -> Self {
        Self { host }
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
            SandboxBackend::Seatbelt => {
                reject_unrenderable_roots(app, policy)?;
                seatbelt_argv(&host.program, policy, program, args)
            }
            SandboxBackend::Bwrap => bwrap_argv(&host.program, policy, program, args),
        })
    }
}

fn reject_unrenderable_roots(app: &str, policy: &SandboxPolicy) -> Result<(), SandboxError> {
    let granted = policy.granted_roots();
    let offending = granted
        .iter()
        .find(|root| root.contains(UNRENDERABLE_PATH_CHARACTERS));
    let Some(root) = offending else {
        return Ok(());
    };
    Err(SandboxError::Unsupported {
        app: app.to_string(),
        reason: format!(
            "writable root '{root}' contains a newline that cannot be expressed in a seatbelt profile"
        ),
    })
}

#[cfg(test)]
#[path = "../tests/sandbox_wrapper_tests.rs"]
mod tests;
