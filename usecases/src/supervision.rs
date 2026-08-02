use thiserror::Error;

use crate::{
    UsecaseError, ports::SpecResolveError, record::ProcessView, selector::AppSelector,
    start::StartOutcome,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SupervisionRequest {
    Start { services: Vec<String> },
    List,
    Describe(AppSelector),
    Stop(AppSelector),
    Restart(AppSelector),
    Delete(AppSelector),
    StopAll,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SupervisionReply {
    Started {
        outcomes: Vec<StartOutcome>,
        refused: Vec<String>,
        reason: Option<String>,
    },
    Listed(Vec<ProcessView>),
    Described(ProcessView),
    Stopped {
        name: String,
    },
    Restarted {
        name: String,
    },
    Deleted {
        name: String,
    },
    StoppedAll {
        names: Vec<String>,
    },
}

#[derive(Debug, Error)]
pub enum SupervisionFailure {
    #[error(transparent)]
    Usecase(#[from] UsecaseError),

    #[error(transparent)]
    Spec(#[from] SpecResolveError),
}

pub type SupervisionOutcome = Result<SupervisionReply, SupervisionFailure>;

impl SupervisionRequest {
    #[must_use]
    pub const fn action(&self) -> &'static str {
        match self {
            Self::Start { services: _ } => "start",
            Self::List => "list",
            Self::Describe(_) => "describe",
            Self::Stop(_) => "stop",
            Self::Restart(_) => "restart",
            Self::Delete(_) => "delete",
            Self::StopAll => "stop_all",
        }
    }

    #[must_use]
    pub fn target(&self) -> String {
        match self {
            Self::Start { services } => services.join(","),
            Self::List | Self::StopAll => String::new(),
            Self::Describe(selector)
            | Self::Stop(selector)
            | Self::Restart(selector)
            | Self::Delete(selector) => selector.to_string(),
        }
    }
}

#[cfg(test)]
#[path = "tests/supervision_tests.rs"]
mod tests;
