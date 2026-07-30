use super::*;

const POLICY: RestartPolicy = RestartPolicy {
    autorestart: true,
    min_uptime_ms: 1000,
    max_restarts: 2,
    restart_delay_ms: 250,
};

#[test]
fn gives_up_when_autorestart_is_disabled() {
    let policy = RestartPolicy {
        autorestart: false,
        ..POLICY
    };
    let decision = decide_restart(policy, 5000, 0);
    assert_eq!(
        decision,
        RestartDecision::GiveUp {
            unstable_restarts: 0
        }
    );
}

#[test]
fn autorestart_disabled_preserves_unstable_counter() {
    let policy = RestartPolicy {
        autorestart: false,
        ..POLICY
    };
    let decision = decide_restart(policy, 10, 7);
    assert_eq!(
        decision,
        RestartDecision::GiveUp {
            unstable_restarts: 7
        }
    );
}

#[test]
fn stable_run_resets_unstable_counter() {
    let decision = decide_restart(POLICY, 5000, 2);
    assert_eq!(
        decision,
        RestartDecision::Restart {
            delay_ms: 250,
            unstable_restarts: 0,
        }
    );
}

#[test]
fn uptime_equal_to_min_uptime_counts_as_stable() {
    let decision = decide_restart(POLICY, 1000, 2);
    assert_eq!(
        decision,
        RestartDecision::Restart {
            delay_ms: 250,
            unstable_restarts: 0,
        }
    );
}

#[test]
fn uptime_one_ms_below_min_uptime_counts_as_unstable() {
    let decision = decide_restart(POLICY, 999, 0);
    assert_eq!(
        decision,
        RestartDecision::Restart {
            delay_ms: 250,
            unstable_restarts: 1,
        }
    );
}

#[test]
fn unstable_run_at_max_restarts_still_restarts() {
    let decision = decide_restart(POLICY, 10, 1);
    assert_eq!(
        decision,
        RestartDecision::Restart {
            delay_ms: 250,
            unstable_restarts: 2,
        }
    );
}

#[test]
fn unstable_run_beyond_max_restarts_gives_up() {
    let decision = decide_restart(POLICY, 10, 2);
    assert_eq!(
        decision,
        RestartDecision::GiveUp {
            unstable_restarts: 3
        }
    );
}

#[test]
fn zero_max_restarts_gives_up_on_first_unstable_exit() {
    let policy = RestartPolicy {
        max_restarts: 0,
        ..POLICY
    };
    let decision = decide_restart(policy, 10, 0);
    assert_eq!(
        decision,
        RestartDecision::GiveUp {
            unstable_restarts: 1
        }
    );
}

#[test]
fn unstable_counter_saturates_instead_of_wrapping() {
    let decision = decide_restart(POLICY, 10, u32::MAX);
    assert_eq!(
        decision,
        RestartDecision::GiveUp {
            unstable_restarts: u32::MAX,
        }
    );
}
