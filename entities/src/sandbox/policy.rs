use thiserror::Error;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SandboxMode {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxPolicy {
    pub mode: SandboxMode,
    pub network: bool,
    pub writable_roots: Vec<String>,
    pub derived_roots: Vec<String>,
}

#[derive(Debug, Eq, PartialEq, Error)]
pub enum PolicyError {
    #[error("cannot accept empty sandbox writable root")]
    EmptyWritableRoot,

    #[error("cannot accept relative sandbox writable root '{0}': must be an absolute path")]
    RelativeWritableRoot(String),

    #[error("cannot accept empty derived sandbox root (from cwd, logs_dir or tmp dir)")]
    EmptyDerivedRoot,

    #[error(
        "cannot accept relative derived sandbox root '{0}' (from cwd, logs_dir or tmp dir): must be an absolute path"
    )]
    RelativeDerivedRoot(String),

    #[error("cannot accept sandbox writable roots under a mode that denies writes")]
    WritableRootsWithoutWriteAccess,
}

impl SandboxMode {
    pub const ALL: [Self; 3] = [Self::ReadOnly, Self::WorkspaceWrite, Self::DangerFullAccess];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::WorkspaceWrite => "workspace-write",
            Self::DangerFullAccess => "danger-full-access",
        }
    }

    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "read-only" => Some(Self::ReadOnly),
            "workspace-write" => Some(Self::WorkspaceWrite),
            "danger-full-access" => Some(Self::DangerFullAccess),
            _ => None,
        }
    }

    #[must_use]
    pub const fn allows_writes(self) -> bool {
        matches!(self, Self::WorkspaceWrite | Self::DangerFullAccess)
    }

    #[must_use]
    pub const fn is_unconfined(self) -> bool {
        matches!(self, Self::DangerFullAccess)
    }
}

impl SandboxPolicy {
    #[must_use]
    pub fn granted_roots(&self) -> Vec<&str> {
        self.writable_roots
            .iter()
            .chain(self.derived_roots.iter())
            .map(String::as_str)
            .collect()
    }
}

pub fn validate_policy(policy: &SandboxPolicy) -> Result<(), PolicyError> {
    let granted = policy.granted_roots();
    if !policy.mode.allows_writes() && !granted.is_empty() {
        return Err(PolicyError::WritableRootsWithoutWriteAccess);
    }

    validate_roots(
        &policy.writable_roots,
        || PolicyError::EmptyWritableRoot,
        PolicyError::RelativeWritableRoot,
    )?;
    validate_roots(
        &policy.derived_roots,
        || PolicyError::EmptyDerivedRoot,
        PolicyError::RelativeDerivedRoot,
    )
}

fn validate_roots(
    roots: &[String],
    empty: fn() -> PolicyError,
    relative: fn(String) -> PolicyError,
) -> Result<(), PolicyError> {
    for root in roots {
        if root.is_empty() {
            return Err(empty());
        }
        if !root.starts_with('/') {
            return Err(relative(root.clone()));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/sandbox_policy_tests.rs"]
mod tests;
