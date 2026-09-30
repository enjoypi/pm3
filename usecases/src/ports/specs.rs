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

#[cfg(test)]
#[path = "../tests/ports_specs_tests.rs"]
mod tests;
