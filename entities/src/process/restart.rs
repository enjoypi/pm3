#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct RestartPolicy {
    pub autorestart: bool,
    pub min_uptime_ms: u64,
    pub max_restarts: u32,
    pub restart_delay_ms: u64,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum RestartDecision {
    Restart {
        delay_ms: u64,
        unstable_restarts: u32,
    },
    GiveUp {
        unstable_restarts: u32,
    },
}

#[must_use]
pub const fn decide_restart(
    policy: RestartPolicy,
    last_uptime_ms: Option<u64>,
    previous_unstable_restarts: u32,
) -> RestartDecision {
    let RestartPolicy {
        autorestart,
        min_uptime_ms,
        max_restarts,
        restart_delay_ms,
    } = policy;

    if !autorestart {
        return RestartDecision::GiveUp {
            unstable_restarts: previous_unstable_restarts,
        };
    }

    let unstable_restarts = match last_uptime_ms {
        Some(uptime_ms) if uptime_ms < min_uptime_ms => {
            previous_unstable_restarts.saturating_add(1)
        }
        Some(_) | None => 0,
    };

    if max_restarts > 0 && unstable_restarts >= max_restarts {
        return RestartDecision::GiveUp { unstable_restarts };
    }

    RestartDecision::Restart {
        delay_ms: restart_delay_ms,
        unstable_restarts,
    }
}

#[cfg(test)]
#[path = "../tests/process_restart_tests.rs"]
mod tests;
