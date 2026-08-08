use super::*;

const POLICY: RestartPolicy = RestartPolicy {
    autorestart: true,
    min_uptime_ms: 1000,
    max_restarts: 2,
    restart_delay_ms: 250,
    max_restart_delay_ms: 15000,
};

#[test]
fn gives_up_when_autorestart_is_disabled() {
    let policy = RestartPolicy {
        autorestart: false,
        ..POLICY
    };
    let decision = decide_restart(policy, Some(5000), 0);
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
    let decision = decide_restart(policy, Some(10), 7);
    assert_eq!(
        decision,
        RestartDecision::GiveUp {
            unstable_restarts: 7
        }
    );
}

#[test]
fn stable_run_resets_unstable_counter() {
    let decision = decide_restart(POLICY, Some(5000), 2);
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
    let decision = decide_restart(POLICY, Some(1000), 2);
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
    let decision = decide_restart(POLICY, Some(999), 0);
    assert_eq!(
        decision,
        RestartDecision::Restart {
            delay_ms: 250,
            unstable_restarts: 1,
        }
    );
}

#[test]
fn unstable_run_reaching_max_restarts_gives_up() {
    let decision = decide_restart(POLICY, Some(10), 1);
    assert_eq!(
        decision,
        RestartDecision::GiveUp {
            unstable_restarts: 2
        }
    );
}

#[test]
fn unstable_run_one_below_max_restarts_still_restarts() {
    let policy = RestartPolicy {
        max_restarts: 3,
        ..POLICY
    };
    let decision = decide_restart(policy, Some(10), 1);
    assert_eq!(
        decision,
        RestartDecision::Restart {
            delay_ms: 500,
            unstable_restarts: 2,
        }
    );
}

#[test]
fn unstable_run_beyond_max_restarts_gives_up() {
    let decision = decide_restart(POLICY, Some(10), 2);
    assert_eq!(
        decision,
        RestartDecision::GiveUp {
            unstable_restarts: 3
        }
    );
}

#[test]
fn zero_max_restarts_disables_the_breaker() {
    let policy = RestartPolicy {
        max_restarts: 0,
        ..POLICY
    };
    let decision = decide_restart(policy, Some(10), 5);
    assert_eq!(
        decision,
        RestartDecision::Restart {
            delay_ms: 8000,
            unstable_restarts: 6,
        }
    );
}

#[test]
fn a_stable_exit_with_zero_max_restarts_still_restarts() {
    let policy = RestartPolicy {
        max_restarts: 0,
        ..POLICY
    };
    let decision = decide_restart(policy, Some(5000), 0);
    assert_eq!(
        decision,
        RestartDecision::Restart {
            delay_ms: 250,
            unstable_restarts: 0,
        }
    );
}

#[test]
fn an_unknown_uptime_does_not_count_as_unstable() {
    let decision = decide_restart(POLICY, None, 1);
    assert_eq!(
        decision,
        RestartDecision::Restart {
            delay_ms: 250,
            unstable_restarts: 0,
        }
    );
}

#[test]
fn unstable_counter_saturates_instead_of_wrapping() {
    let decision = decide_restart(POLICY, Some(10), u32::MAX);
    assert_eq!(
        decision,
        RestartDecision::GiveUp {
            unstable_restarts: u32::MAX,
        }
    );
}

#[test]
fn the_first_unstable_restart_keeps_the_base_delay() {
    let decision = decide_restart(POLICY, Some(10), 0);
    assert_eq!(
        decision,
        RestartDecision::Restart {
            delay_ms: 250,
            unstable_restarts: 1,
        }
    );
}

#[test]
fn the_third_unstable_restart_quadruples_the_delay() {
    let policy = RestartPolicy {
        max_restarts: 0,
        ..POLICY
    };
    let decision = decide_restart(policy, Some(10), 2);
    assert_eq!(
        decision,
        RestartDecision::Restart {
            delay_ms: 1000,
            unstable_restarts: 3,
        }
    );
}

#[test]
fn the_delay_is_capped_at_the_configured_maximum() {
    let policy = RestartPolicy {
        max_restarts: 0,
        max_restart_delay_ms: 600,
        ..POLICY
    };
    let decision = decide_restart(policy, Some(10), 2);
    assert_eq!(
        decision,
        RestartDecision::Restart {
            delay_ms: 600,
            unstable_restarts: 3,
        }
    );
}

#[test]
fn a_zero_base_delay_stays_zero() {
    let policy = RestartPolicy {
        max_restarts: 0,
        restart_delay_ms: 0,
        ..POLICY
    };
    let decision = decide_restart(policy, Some(10), 4);
    assert_eq!(
        decision,
        RestartDecision::Restart {
            delay_ms: 0,
            unstable_restarts: 5,
        }
    );
}

#[test]
fn the_delay_saturates_instead_of_wrapping() {
    let policy = RestartPolicy {
        max_restarts: 0,
        restart_delay_ms: u64::MAX / 2,
        max_restart_delay_ms: u64::MAX,
        ..POLICY
    };
    let decision = decide_restart(policy, Some(10), 2);
    assert_eq!(
        decision,
        RestartDecision::Restart {
            delay_ms: u64::MAX,
            unstable_restarts: 3,
        }
    );
}

#[test]
fn a_base_delay_above_the_cap_falls_back_to_the_cap() {
    let policy = RestartPolicy {
        restart_delay_ms: 20000,
        ..POLICY
    };
    let decision = decide_restart(policy, Some(5000), 0);
    assert_eq!(
        decision,
        RestartDecision::Restart {
            delay_ms: 15000,
            unstable_restarts: 0,
        }
    );
}
