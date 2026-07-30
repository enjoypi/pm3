#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessStatus {
    Launching,
    Online,
    Stopping,
    Stopped,
    Errored,
}

impl ProcessStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Launching => "launching",
            Self::Online => "online",
            Self::Stopping => "stopping",
            Self::Stopped => "stopped",
            Self::Errored => "errored",
        }
    }

    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "launching" => Some(Self::Launching),
            "online" => Some(Self::Online),
            "stopping" => Some(Self::Stopping),
            "stopped" => Some(Self::Stopped),
            "errored" => Some(Self::Errored),
            _ => None,
        }
    }

    #[must_use]
    pub const fn is_running(self) -> bool {
        matches!(self, Self::Launching | Self::Online)
    }

    #[must_use]
    pub const fn is_shutting_down(self) -> bool {
        matches!(self, Self::Stopping)
    }
}

#[cfg(test)]
#[path = "../tests/process_status_tests.rs"]
mod tests;
