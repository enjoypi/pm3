use entities::ReadyProbe;
use thiserror::Error;

use crate::{
    UsecaseError, ports::SpecResolveError, record::ProcessView, selector::AppSelector,
    start::StartOutcome,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SupervisionRequest {
    Start {
        services: Vec<String>,
    },
    List,
    Describe(AppSelector),
    Stop(AppSelector),
    Restart(AppSelector),
    Delete(AppSelector),
    Reset(AppSelector),
    Signal {
        selector: AppSelector,
        signal: String,
    },
    StopAll,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SupervisionReply {
    Started {
        outcomes: Vec<StartOutcome>,
        refused: Vec<String>,
        reason: Option<String>,
        unsaved: Option<String>,
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
    Reset {
        name: String,
    },
    Signalled {
        name: String,
        signal: String,
    },
    StoppedAll {
        names: Vec<String>,
    },
    RestartedAll {
        names: Vec<String>,
    },
    DeletedAll {
        names: Vec<String>,
    },
    ResetAll {
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
            Self::Reset(_) => "reset",
            Self::Signal { .. } => "signal",
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
            | Self::Delete(selector)
            | Self::Reset(selector)
            | Self::Signal {
                selector,
                signal: _,
            } => selector.to_string(),
        }
    }
}

#[cfg(test)]
#[path = "tests/supervision_tests.rs"]
mod tests;
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SupervisionEffect {
    ScheduleMemorySample {
        delay_ms: u64,
    },
    ArmTimer {
        name: String,
        fire_at_ms: u64,
        delay_ms: u64,
    },
    DisarmTimer {
        name: String,
    },
    ScheduleRestart {
        name: String,
        delay_ms: u64,
    },
    CancelRestart {
        name: String,
    },
    ScheduleForceKill {
        name: String,
        generation: u64,
        pid: u32,
        token: Option<String>,
        delay_ms: u64,
    },
    CancelForceKill {
        name: String,
    },
    WatchExit {
        name: String,
        generation: u64,
        pid: u32,
        token: Option<String>,
    },
    ScheduleLogRotate {
        delay_ms: u64,
    },
    AwaitReady {
        name: String,
        generation: u64,
        probe: ReadyProbe,
        timeout_ms: u64,
        interval_ms: u64,
    },
    CancelReady {
        name: String,
    },
}
