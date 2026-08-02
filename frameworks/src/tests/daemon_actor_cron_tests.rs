use adapters::StartKind;

use super::{shared::*, test_helpers::*, *};

#[tokio::test]
async fn registering_a_task_arms_a_timer_without_spawning() {
    let mut harness = harness();
    let outcome = start_scheduled(&mut harness, "tick", "* * * * *").await;
    assert_eq!(outcome.kind, StartKind::Scheduled);
    assert_eq!(outcome.pid, None);
    assert!(armed_fire(&mut harness, "tick").await > 0);
}

#[tokio::test]
async fn a_due_timer_launches_the_task_and_arms_the_next_one() {
    let mut harness = harness();
    start_scheduled(&mut harness, "tick", "* * * * *").await;
    let first = armed_fire(&mut harness, "tick").await;

    harness.daemon.on_fire("tick", first).await;

    let view = described(&mut harness, "tick").await;
    assert_eq!(view.status, adapters::ProcessStatus::Online);
    assert!(view.pid.is_some(), "the fire should have spawned the task");
    assert!(
        view.next_fire_ms.is_some_and(|next| next >= first),
        "firing must arm the following cycle, got: {:?}",
        view.next_fire_ms
    );
}

#[tokio::test]
async fn a_stale_timer_is_ignored() {
    let mut harness = harness();
    start_scheduled(&mut harness, "tick", "* * * * *").await;
    let armed = armed_fire(&mut harness, "tick").await;

    harness
        .daemon
        .on_fire("tick", armed.saturating_sub(1))
        .await;

    let view = described(&mut harness, "tick").await;
    assert_eq!(view.pid, None, "a stale timer must not spawn anything");
}

#[tokio::test]
async fn stopping_a_task_disarms_its_timer() {
    let mut harness = harness();
    start_scheduled(&mut harness, "tick", "* * * * *").await;
    let armed = armed_fire(&mut harness, "tick").await;

    harness
        .daemon
        .handle(DaemonRequest::Stop(selector("tick")))
        .await
        .expect("should stop");

    assert_eq!(described(&mut harness, "tick").await.next_fire_ms, None);
    harness.daemon.on_fire("tick", armed).await;
    assert_eq!(
        described(&mut harness, "tick").await.pid,
        None,
        "a disarmed task must stay put"
    );
}

#[tokio::test]
async fn deleting_a_task_disarms_its_timer() {
    let mut harness = harness();
    start_scheduled(&mut harness, "tick", "* * * * *").await;
    harness
        .daemon
        .handle(DaemonRequest::Delete(selector("tick")))
        .await
        .expect("should delete");
    assert_eq!(listed(&mut harness).await, 0);
}

#[tokio::test]
async fn an_unschedulable_expression_leaves_no_timer() {
    let mut harness = harness();
    start_scheduled(&mut harness, "tick", "0 0 30 2 *").await;
    assert_eq!(described(&mut harness, "tick").await.next_fire_ms, None);
}

#[tokio::test]
async fn an_app_without_a_schedule_never_arms_a_timer() {
    let mut harness = harness();
    start_one(&mut harness, "web", SLEEPER).await;
    assert_eq!(described(&mut harness, "web").await.next_fire_ms, None);
}

#[tokio::test(start_paused = true)]
async fn an_armed_timer_queues_its_fire_when_it_comes_due() {
    let mut harness = harness();
    start_scheduled(&mut harness, "tick", "* * * * *").await;
    let armed = armed_fire(&mut harness, "tick").await;

    let queued = harness
        .events
        .recv()
        .await
        .expect("the timer should queue a fire");
    assert!(
        matches!(queued, DaemonEvent::Fire { ref name, fire_at_ms } if name == "tick" && fire_at_ms == armed),
        "unexpected event: {queued:?}"
    );
}

#[tokio::test]
async fn taking_over_saved_apps_re_arms_their_timers() {
    let mut harness = harness();
    start_scheduled(&mut harness, "tick", "* * * * *").await;

    harness.daemon.resurrect_saved_apps().await;

    assert!(
        described(&mut harness, "tick").await.next_fire_ms.is_some(),
        "a reclaimed task keeps its schedule"
    );
}

#[tokio::test]
async fn a_task_stopped_on_purpose_stays_disarmed_across_a_daemon_restart() {
    let mut harness = harness();
    start_scheduled(&mut harness, "tick", "* * * * *").await;
    harness
        .daemon
        .handle(DaemonRequest::Stop(selector("tick")))
        .await
        .expect("should stop");

    harness.daemon.resurrect_saved_apps().await;

    assert_eq!(
        described(&mut harness, "tick").await.next_fire_ms,
        None,
        "a task the operator stopped must not revive itself"
    );
}

#[tokio::test]
async fn stopping_everything_disarms_every_timer() {
    let mut harness = harness();
    start_scheduled(&mut harness, "tick", "* * * * *").await;
    let armed = armed_fire(&mut harness, "tick").await;

    harness
        .daemon
        .handle(DaemonRequest::StopAll)
        .await
        .expect("should stop everything");

    assert_eq!(described(&mut harness, "tick").await.next_fire_ms, None);
    harness.daemon.on_fire("tick", armed).await;
    assert_eq!(
        described(&mut harness, "tick").await.pid,
        None,
        "a fire queued behind stop-all must not spawn anything"
    );
}

#[tokio::test]
async fn everything_stopped_together_stays_disarmed_across_a_daemon_restart() {
    let mut harness = harness();
    start_scheduled(&mut harness, "tick", "* * * * *").await;
    harness
        .daemon
        .handle(DaemonRequest::StopAll)
        .await
        .expect("should stop everything");

    harness.daemon.resurrect_saved_apps().await;

    assert_eq!(described(&mut harness, "tick").await.next_fire_ms, None);
}

#[tokio::test]
async fn a_cron_fire_cancels_the_delayed_restart_it_replaces() {
    let mut harness = harness();
    start_scheduled(&mut harness, "tick", "* * * * *").await;
    harness.daemon.board.schedule_restart("tick", 60_000);
    let armed = armed_fire(&mut harness, "tick").await;
    harness.daemon.on_fire("tick", armed).await;
    let pid = described(&mut harness, "tick")
        .await
        .pid
        .expect("the fire should have spawned the task");

    harness.daemon.on_restart("tick").await;

    let view = described(&mut harness, "tick").await;
    assert_eq!(
        (view.status, view.pid),
        (adapters::ProcessStatus::Online, Some(pid)),
        "a stale delayed restart must not kill the instance the fire started"
    );
}

#[tokio::test]
async fn re_registering_a_running_app_without_its_schedule_disarms_the_timer() {
    let mut harness = harness();
    scheduled_online_apps_file(&harness, "web", SLEEPER, "* * * * *");
    let request = DaemonRequest::Start {
        services: vec!["web".to_string()],
    };
    harness
        .daemon
        .handle(request.clone())
        .await
        .expect("first start");
    let armed = armed_fire(&mut harness, "web").await;
    let pid = described(&mut harness, "web").await.pid;

    apps_file(&harness, "web", SLEEPER);
    harness.daemon.handle(request).await.expect("second start");

    assert_eq!(
        described(&mut harness, "web").await.next_fire_ms,
        None,
        "a removed schedule must disarm the timer"
    );
    harness.daemon.on_fire("web", armed).await;
    assert_eq!(
        described(&mut harness, "web").await.pid,
        pid,
        "a disarmed timer must not restart the app"
    );
}

#[tokio::test]
async fn re_registering_a_running_app_with_a_new_schedule_arms_a_timer() {
    let mut harness = harness();
    start_one(&mut harness, "web", SLEEPER).await;
    assert_eq!(described(&mut harness, "web").await.next_fire_ms, None);

    scheduled_online_apps_file(&harness, "web", SLEEPER, "* * * * *");
    harness
        .daemon
        .handle(DaemonRequest::Start {
            services: vec!["web".to_string()],
        })
        .await
        .expect("second start");

    assert!(
        described(&mut harness, "web").await.next_fire_ms.is_some(),
        "a schedule added to a running app must arm a timer"
    );
}

#[tokio::test]
async fn re_registering_a_running_app_with_a_changed_schedule_rearms_the_timer() {
    let mut harness = harness();
    scheduled_online_apps_file(&harness, "web", SLEEPER, "* * * * *");
    let request = DaemonRequest::Start {
        services: vec!["web".to_string()],
    };
    harness
        .daemon
        .handle(request.clone())
        .await
        .expect("first start");
    let first = armed_fire(&mut harness, "web").await;

    scheduled_online_apps_file(&harness, "web", SLEEPER, "0 0 29 2 *");
    harness.daemon.handle(request).await.expect("second start");

    assert_ne!(
        armed_fire(&mut harness, "web").await,
        first,
        "a changed schedule must move the next fire"
    );
}
