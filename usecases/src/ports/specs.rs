use std::future::Future;

use entities::AppSpec;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SpecResolveError {
    #[error("{reason}")]
    Missing { name: String, reason: String },

    #[error("{reason}")]
    Unusable { name: String, reason: String },
}

pub trait SpecResolver: Send + Sync {
    fn prepare(&self, name: &str)
    -> impl Future<Output = Result<AppSpec, SpecResolveError>> + Send;
}

impl SpecResolveError {
    #[must_use]
    pub fn app(&self) -> &str {
        match self {
            Self::Missing { name, reason: _ } | Self::Unusable { name, reason: _ } => name,
        }
    }
}

#[cfg(test)]
#[path = "../tests/ports_specs_tests.rs"]
mod tests;
