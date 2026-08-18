use thiserror::Error;

use super::roots::{covers_path, normalize_root};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SandboxMode {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ReadScope {
    Full,
    Minimal,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxPolicy {
    pub mode: SandboxMode,
    pub read: ReadScope,
    pub network: bool,
    pub writable_roots: Vec<String>,
    pub readable_roots: Vec<String>,
    pub derived_readable_roots: Vec<String>,
    pub derived_roots: Vec<String>,
    pub unreadable_roots: Vec<String>,
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

    #[error("cannot accept empty sandbox readable root")]
    EmptyReadableRoot,

    #[error("cannot accept relative sandbox readable root '{0}': must be an absolute path")]
    RelativeReadableRoot(String),

    #[error("cannot accept sandbox writable roots under a mode that denies writes")]
    WritableRootsWithoutWriteAccess,

    #[error(
        "cannot accept sandbox writable root '{root}': it would hand the service '{hidden}', which pm3 keeps out of every sandbox"
    )]
    WritableRootCoversHiddenRoot { root: String, hidden: String },

    #[error(
        "cannot accept sandbox writable root '{0}': pm3.sandbox.forbidden_writable_roots refuses it because granting it would leave the sandbox with nothing to enforce"
    )]
    ForbiddenWritableRoot(String),
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

impl ReadScope {
    pub const ALL: [Self; 2] = [Self::Full, Self::Minimal];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Minimal => "minimal",
        }
    }

    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "full" => Some(Self::Full),
            "minimal" => Some(Self::Minimal),
            _ => None,
        }
    }

    #[must_use]
    pub const fn confines_reads(self) -> bool {
        matches!(self, Self::Minimal)
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

    #[must_use]
    pub fn readable_grants(&self) -> Vec<&str> {
        self.readable_roots
            .iter()
            .chain(self.derived_readable_roots.iter())
            .map(String::as_str)
            .collect()
    }

    #[must_use]
    pub fn hidden_paths(&self) -> Vec<&str> {
        self.unreadable_roots.iter().map(String::as_str).collect()
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
    )?;
    validate_roots(
        &policy.readable_roots,
        || PolicyError::EmptyReadableRoot,
        PolicyError::RelativeReadableRoot,
    )?;
    validate_hidden_roots_stay_hidden(policy)
}

pub fn validate_forbidden_roots(
    policy: &SandboxPolicy,
    forbidden: &[String],
) -> Result<(), PolicyError> {
    policy
        .writable_roots
        .iter()
        .find(|root| {
            forbidden
                .iter()
                .any(|denied| normalize_root(denied) == normalize_root(root))
        })
        .map_or(Ok(()), |root| {
            Err(PolicyError::ForbiddenWritableRoot(root.clone()))
        })
}

fn validate_hidden_roots_stay_hidden(policy: &SandboxPolicy) -> Result<(), PolicyError> {
    for root in policy.granted_roots() {
        let covered = policy
            .unreadable_roots
            .iter()
            .find(|hidden| covers_path(root, hidden));
        if let Some(hidden) = covered {
            return Err(PolicyError::WritableRootCoversHiddenRoot {
                root: root.to_string(),
                hidden: hidden.clone(),
            });
        }
    }
    Ok(())
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
