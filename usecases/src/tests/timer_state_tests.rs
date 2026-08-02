use super::*;

#[test]
fn an_unarmed_app_advertises_no_next_fire() {
    assert!(TimerState::new().next_fire_of("nightly").is_none());
}

#[test]
fn arming_records_the_instant_the_next_fire_is_due() {
    let mut state = TimerState::new();
    state.arm("nightly", 1_700_000_040_000);
    assert_eq!(state.next_fire_of("nightly"), Some(1_700_000_040_000));
}

#[test]
fn re_arming_replaces_the_instant() {
    let mut state = TimerState::new();
    state.arm("nightly", 1000);
    state.arm("nightly", 2000);
    assert_eq!(state.next_fire_of("nightly"), Some(2000));
}

#[test]
fn disarming_an_armed_app_reports_that_it_was_armed() {
    let mut state = TimerState::new();
    state.arm("nightly", 1000);
    assert!(state.disarm("nightly"));
    assert!(state.next_fire_of("nightly").is_none());
}

#[test]
fn disarming_an_unarmed_app_reports_nothing_to_do() {
    assert!(!TimerState::new().disarm("nightly"));
}

#[test]
fn disarming_everything_names_what_it_disarmed() {
    let mut state = TimerState::new();
    state.arm("nightly", 1000);
    state.arm("hourly", 2000);
    let mut disarmed = state.disarm_all();
    disarmed.sort();
    assert_eq!(disarmed, vec!["hourly".to_string(), "nightly".to_string()]);
    assert!(state.next_fire_of("nightly").is_none());
}

#[test]
fn a_fire_is_due_only_for_the_instant_that_was_armed() {
    let mut state = TimerState::new();
    state.arm("nightly", 1000);
    assert!(state.fire_is_due("nightly", 1000));
    assert!(!state.fire_is_due("nightly", 999));
    assert!(!state.fire_is_due("hourly", 1000));
}

#[test]
fn a_queued_restart_can_be_claimed_once() {
    let mut state = TimerState::new();
    state.queue_restart("web");
    assert!(state.claim_restart("web"));
    assert!(!state.claim_restart("web"));
}

#[test]
fn a_restart_that_was_never_queued_cannot_be_claimed() {
    assert!(!TimerState::new().claim_restart("web"));
}

#[test]
fn cancelling_a_queued_restart_reports_that_one_was_queued() {
    let mut state = TimerState::new();
    state.queue_restart("web");
    assert!(state.cancel_restart("web"));
    assert!(!state.cancel_restart("web"));
}

#[test]
fn cancelling_every_restart_names_what_it_cancelled() {
    let mut state = TimerState::new();
    state.queue_restart("web");
    state.queue_restart("api");
    let mut cancelled = state.cancel_all_restarts();
    cancelled.sort();
    assert_eq!(cancelled, vec!["api".to_string(), "web".to_string()]);
    assert!(!state.claim_restart("web"));
}

#[test]
fn a_queued_force_kill_is_visible_until_it_is_cancelled() {
    let mut state = TimerState::new();
    state.queue_force_kill("web");
    assert!(state.has_force_kill("web"));
    assert!(state.cancel_force_kill("web"));
    assert!(!state.has_force_kill("web"));
}

#[test]
fn cancelling_a_force_kill_that_was_never_queued_reports_nothing_to_do() {
    assert!(!TimerState::new().cancel_force_kill("web"));
}

#[test]
fn an_app_that_never_launched_sits_at_the_first_generation() {
    let state = TimerState::new();
    assert!(state.is_current("web", 0));
}

#[test]
fn bumping_hands_out_a_fresh_generation_each_time() {
    let mut state = TimerState::new();
    let first = state.bump("web");
    let second = state.bump("web");
    assert_ne!(first, second);
    assert!(state.is_current("web", second));
    assert!(!state.is_current("web", first));
}

#[test]
fn generations_do_not_collide_across_apps() {
    let mut state = TimerState::new();
    let web = state.bump("web");
    let api = state.bump("api");
    assert_ne!(web, api);
    assert!(state.is_current("web", web));
    assert!(state.is_current("api", api));
}

#[test]
fn forgetting_a_generation_sends_the_app_back_to_the_first_one() {
    let mut state = TimerState::new();
    let bumped = state.bump("web");
    state.forget_generation("web");
    assert!(!state.is_current("web", bumped));
    assert!(state.is_current("web", 0));
}
