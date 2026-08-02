pub mod process;
pub mod sandbox;

use thiserror::Error;

pub use self::{
    process::{
        AppSpec, DependencyError, DependencyNode, ProcessIdentity, ProcessRuntime, ProcessStatus,
        RestartDecision, RestartPolicy, RuntimeError, SpecError, decide_restart, topo_sort,
        validate_app_name, validate_spec,
    },
    sandbox::{PolicyError, SandboxMode, SandboxPolicy, validate_policy},
};

#[derive(Debug, Eq, PartialEq, Error)]
pub enum EntityError {
    #[error(transparent)]
    Spec(#[from] SpecError),

    #[error(transparent)]
    Dependency(#[from] DependencyError),

    #[error(transparent)]
    Policy(#[from] PolicyError),

    #[error(transparent)]
    Runtime(#[from] RuntimeError),
}

pub type Result<T> = std::result::Result<T, EntityError>;

#[cfg(test)]
#[path = "tests/lib_tests.rs"]
mod tests;
