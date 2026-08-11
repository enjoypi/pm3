use entities::ProcessStatus;

use super::*;

fn stale_survivor(ports: &FakePorts, name: &str, pm_id: u32, pid: u32) -> ProcessRecord {
    let mut record = stored_record(name, pm_id, ProcessStatus::Online);
    record.runtime.mark_launched(pid, 1000);
    record.runtime.identity = Some(ProcessIdentity {
        token: live_token(pid),
        launch_digest: "stale".to_string(),
        binary_digest: format!("file:{}", record.spec.script),
    });
    ports.seed_live(pid, &live_token(pid));
    record
}

#[tokio::test]
async fn stale_survivors_are_evicted_together_before_any_waits() {
    let ports = FakePorts::new(1000);
    ports.seed_stored(vec![
        stale_survivor(&ports, "api", 0, 7),
        stale_survivor(&ports, "web", 1, 8),
    ]);
    ports.make_stubborn(7);
    ports.make_stubborn(8);
    ports.slow_waits();

    resurrected(&ports).await;

    let events = ports.events();
    let second_signal = events
        .iter()
        .position(|event| event == "terminate:8")
        .expect("the second survivor should be signalled");
    let first_wait = events
        .iter()
        .position(|event| event == "wait:7")
        .expect("the first survivor should be awaited");
    assert!(
        second_signal < first_wait,
        "evictions must overlap instead of draining one survivor at a time: {events:?}"
    );
    assert_eq!(ports.spawned_names(), vec!["api", "web"]);
}

#[tokio::test]
async fn a_pid_recycled_after_the_verdict_is_spared_the_signal() {
    let ports = FakePorts::new(1000);
    let mut record = survivor(&ports, "api");
    record.runtime.identity = Some(ProcessIdentity {
        launch_digest: "stale".to_string(),
        ..expected_identity(&ports, &record)
    });
    ports.seed_stored(vec![record]);
    ports.recycle_after_probe(SURVIVOR_PID);
    resurrected(&ports).await;
    assert!(
        ports.terminated().is_empty(),
        "got: {:?}",
        ports.terminated()
    );
    assert!(ports.force_killed().is_empty());
    assert_eq!(ports.spawned_names(), vec!["api"]);
}

#[tokio::test]
async fn a_pid_gone_after_the_verdict_is_not_signalled() {
    let ports = FakePorts::new(1000);
    let mut record = survivor(&ports, "api");
    record.runtime.identity = Some(ProcessIdentity {
        launch_digest: "stale".to_string(),
        ..expected_identity(&ports, &record)
    });
    ports.seed_stored(vec![record]);
    ports.vanish_after_probe(SURVIVOR_PID);
    resurrected(&ports).await;
    assert!(
        ports.terminated().is_empty(),
        "got: {:?}",
        ports.terminated()
    );
    assert_eq!(ports.spawned_names(), vec!["api"]);
}

#[tokio::test]
async fn a_pid_recycled_while_draining_is_spared_the_force_kill() {
    let ports = FakePorts::new(1000);
    let mut record = survivor(&ports, "api");
    record.runtime.identity = Some(ProcessIdentity {
        launch_digest: "stale".to_string(),
        ..expected_identity(&ports, &record)
    });
    ports.seed_stored(vec![record]);
    ports.recycle_on_signal(SURVIVOR_PID);
    resurrected(&ports).await;
    assert!(
        ports.force_killed().is_empty(),
        "got: {:?}",
        ports.force_killed()
    );
    assert_eq!(ports.spawned_names(), vec!["api"]);
}

#[tokio::test]
async fn a_stale_survivor_is_stopped_before_its_replacement_starts() {
    let ports = FakePorts::new(1000);
    let mut record = survivor(&ports, "api");
    record.runtime.identity = Some(ProcessIdentity {
        launch_digest: "stale".to_string(),
        ..expected_identity(&ports, &record)
    });
    ports.seed_stored(vec![record]);
    resurrected(&ports).await;
    assert_eq!(ports.terminated(), vec![SURVIVOR_PID]);
}

#[tokio::test]
async fn a_replacement_waits_for_the_stale_survivor_to_leave() {
    let ports = FakePorts::new(1000);
    let mut record = survivor(&ports, "api");
    record.runtime.identity = Some(ProcessIdentity {
        launch_digest: "stale".to_string(),
        ..expected_identity(&ports, &record)
    });
    ports.seed_stored(vec![record]);
    resurrected(&ports).await;
    assert_eq!(ports.waited(), vec![SURVIVOR_PID]);
    assert!(ports.force_killed().is_empty());
}

#[tokio::test]
async fn a_stubborn_stale_survivor_is_force_killed_before_the_replacement_starts() {
    let ports = FakePorts::new(1000);
    let mut record = survivor(&ports, "api");
    record.runtime.identity = Some(ProcessIdentity {
        launch_digest: "stale".to_string(),
        ..expected_identity(&ports, &record)
    });
    ports.seed_stored(vec![record]);
    ports.make_stubborn(SURVIVOR_PID);
    resurrected(&ports).await;
    assert_eq!(ports.force_killed(), vec![SURVIVOR_PID]);
    assert_eq!(ports.spawned_names(), vec!["api"]);
}

#[tokio::test]
async fn a_refused_force_kill_does_not_block_the_replacement() {
    let ports = FakePorts::new(1000);
    let mut record = survivor(&ports, "api");
    record.runtime.identity = Some(ProcessIdentity {
        launch_digest: "stale".to_string(),
        ..expected_identity(&ports, &record)
    });
    ports.seed_stored(vec![record]);
    ports.make_stubborn(SURVIVOR_PID);
    ports.fail_force_kill_for(SURVIVOR_PID);
    resurrected(&ports).await;
    assert!(ports.force_killed().is_empty());
    assert_eq!(ports.spawned_names(), vec!["api"]);
}

#[tokio::test]
async fn a_survivor_that_already_left_is_not_signalled_again() {
    let ports = FakePorts::new(1000);
    ports.seed_stored(vec![survivor(&ports, "api")]);
    ports.kill_silently(SURVIVOR_PID);
    resurrected(&ports).await;
    assert!(ports.terminated().is_empty());
}

#[tokio::test]
async fn a_stale_survivor_that_refuses_the_signal_still_gets_a_replacement() {
    let ports = FakePorts::new(1000);
    let mut record = survivor(&ports, "api");
    record.runtime.identity = Some(ProcessIdentity {
        binary_digest: "stale".to_string(),
        ..expected_identity(&ports, &record)
    });
    ports.seed_stored(vec![record]);
    ports.fail_signal_for(SURVIVOR_PID);
    resurrected(&ports).await;
    assert_eq!(ports.spawned_names(), vec!["api"]);
}

#[tokio::test]
async fn a_digest_read_failure_keeps_the_confirmed_survivor_running() {
    let ports = FakePorts::new(1000);
    ports.seed_stored(vec![survivor(&ports, "api")]);
    ports.fail_file_digest_for("/usr/bin/true");
    let outcomes = resurrect(&mut ProcessTable::new(), LOGS_DIR, KILL_TIMEOUT_MS, &ports)
        .await
        .expect("resurrect should succeed");
    assert_eq!(outcomes[0].kind, StartKind::Adopted);
    assert!(ports.spawned_names().is_empty());
    assert!(ports.terminated().is_empty());
}

#[tokio::test]
async fn a_service_that_must_respawn_without_a_sandbox_is_skipped() {
    let ports = FakePorts::new(1000);
    let mut record = survivor(&ports, "api");
    record.runtime.identity = None;
    ports.seed_stored(vec![record]);
    ports.fail_wrap_for("api");
    let outcomes = resurrect(&mut ProcessTable::new(), LOGS_DIR, KILL_TIMEOUT_MS, &ports)
        .await
        .expect("an unwrappable service must not abort the whole recovery");
    assert!(outcomes.is_empty());
}

#[tokio::test]
async fn a_live_service_is_reclaimed_even_when_the_sandbox_can_no_longer_wrap() {
    let ports = FakePorts::new(1000);
    ports.seed_stored(vec![survivor(&ports, "api")]);
    ports.fail_wrap_for("api");
    let table = resurrected(&ports).await;
    let record = table
        .find(&AppSelector::Name("api".to_string()))
        .expect("record present");
    assert_eq!(record.runtime.pid, Some(SURVIVOR_PID));
    assert!(
        ports.spawned_names().is_empty(),
        "a reclaimed process needs no fresh wrapping"
    );
}

#[tokio::test]
async fn a_dump_written_before_identities_existed_restarts_everything() {
    let ports = FakePorts::new(1000);
    let mut record = survivor(&ports, "api");
    record.runtime.identity = None;
    ports.seed_stored(vec![record]);
    resurrected(&ports).await;
    assert_eq!(ports.spawned_names(), vec!["api"]);
}

#[tokio::test]
async fn reclaimed_and_restarted_services_can_be_mixed() {
    let ports = FakePorts::new(1000);
    let kept = survivor(&ports, "api");
    let mut lost = stored_record("web", 1, ProcessStatus::Online);
    lost.runtime.identity = None;
    ports.seed_stored(vec![kept, lost]);
    let outcomes = resurrect(&mut ProcessTable::new(), LOGS_DIR, KILL_TIMEOUT_MS, &ports)
        .await
        .expect("resurrect should succeed");
    let kinds: Vec<(&str, StartKind)> = outcomes
        .iter()
        .map(|outcome| (outcome.name.as_str(), outcome.kind))
        .collect();
    assert_eq!(
        kinds,
        vec![("api", StartKind::Adopted), ("web", StartKind::Spawned)]
    );
}

fn cycle(ports: &FakePorts) {
    let mut first = stored_record("a", 1, ProcessStatus::Stopped);
    first.spec = spec_with_deps("a", &["b"]);
    let mut second = stored_record("b", 2, ProcessStatus::Stopped);
    second.spec = spec_with_deps("b", &["a"]);
    ports.seed_stored(vec![survivor(ports, "api"), first, second]);
}

#[tokio::test]
async fn a_survivor_pm3_cannot_probe_is_replaced_rather_than_trusted() {
    let ports = FakePorts::new(1000);
    ports.seed_stored(vec![survivor(&ports, "api")]);
    ports.break_probe_for(SURVIVOR_PID);
    let outcomes = resurrect(&mut ProcessTable::new(), LOGS_DIR, KILL_TIMEOUT_MS, &ports)
        .await
        .expect("resurrect should succeed");
    assert_eq!(outcomes[0].kind, StartKind::Spawned);
}

#[tokio::test]
async fn a_survivor_pm3_cannot_probe_is_stopped_before_its_replacement_starts() {
    let ports = FakePorts::new(1000);
    ports.seed_stored(vec![survivor(&ports, "api")]);
    ports.break_probe_for(SURVIVOR_PID);
    resurrect(&mut ProcessTable::new(), LOGS_DIR, KILL_TIMEOUT_MS, &ports)
        .await
        .expect("resurrect should succeed");
    assert_eq!(ports.terminated(), vec![SURVIVOR_PID]);
}

#[tokio::test]
async fn an_unorderable_state_file_still_reclaims_the_survivors() {
    let ports = FakePorts::new(1000);
    cycle(&ports);
    let outcomes = resurrect(&mut ProcessTable::new(), LOGS_DIR, KILL_TIMEOUT_MS, &ports)
        .await
        .expect("a broken dependency graph must not abandon live services");
    let names: Vec<&str> = outcomes
        .iter()
        .map(|outcome| outcome.name.as_str())
        .collect();
    assert_eq!(names, vec!["api"]);
}

#[tokio::test]
async fn an_unorderable_state_file_still_persists_the_table() {
    let ports = FakePorts::new(1000);
    cycle(&ports);
    resurrect(&mut ProcessTable::new(), LOGS_DIR, KILL_TIMEOUT_MS, &ports)
        .await
        .expect("a broken dependency graph must not abandon live services");
    assert_eq!(ports.save_count(), 1);
}

#[tokio::test]
async fn a_service_that_cannot_respawn_does_not_abandon_the_rest() {
    let ports = FakePorts::new(1000);
    ports.fail_spawn_for("web");
    ports.seed_stored(vec![
        survivor(&ports, "api"),
        stored_record("web", 1, ProcessStatus::Online),
    ]);
    let outcomes = resurrect(&mut ProcessTable::new(), LOGS_DIR, KILL_TIMEOUT_MS, &ports)
        .await
        .expect("one broken service must not abandon the ones already reclaimed");
    let names: Vec<&str> = outcomes
        .iter()
        .map(|outcome| outcome.name.as_str())
        .collect();
    assert_eq!(names, vec!["api"]);
}

#[tokio::test]
async fn a_persistence_failure_still_reports_the_services_it_reclaimed() {
    let ports = FakePorts::new(1000);
    ports.seed_stored(vec![survivor(&ports, "api")]);
    ports.fail_save();
    let outcomes = resurrect(&mut ProcessTable::new(), LOGS_DIR, KILL_TIMEOUT_MS, &ports)
        .await
        .expect("a persistence failure must not hide the services already reclaimed");
    assert_eq!(outcomes.len(), 1);
}

#[tokio::test]
async fn a_survivor_without_an_identity_is_signalled_by_pid_not_by_group() {
    let ports = FakePorts::new(1000);
    let mut record = survivor(&ports, "api");
    record.runtime.identity = None;
    ports.seed_stored(vec![record]);
    resurrected(&ports).await;
    assert!(
        ports
            .signal_scopes()
            .iter()
            .any(|(pid, scope)| *pid == SURVIVOR_PID && *scope == SignalScope::SinglePid),
        "an unverified pid must not take the whole process group down: {:?}",
        ports.signal_scopes()
    );
}

#[tokio::test]
async fn a_survivor_without_an_identity_that_already_left_is_not_signalled() {
    let ports = FakePorts::new(1000);
    let mut record = survivor(&ports, "api");
    record.runtime.identity = None;
    ports.seed_stored(vec![record]);
    ports.hide_from_probe(SURVIVOR_PID);
    resurrected(&ports).await;
    assert!(
        ports.terminated().is_empty(),
        "got: {:?}",
        ports.terminated()
    );
}

#[tokio::test]
async fn a_confirmed_survivor_is_still_signalled_through_its_process_group() {
    let ports = FakePorts::new(1000);
    let mut record = survivor(&ports, "api");
    record.runtime.identity = Some(ProcessIdentity {
        launch_digest: "stale".to_string(),
        ..expected_identity(&ports, &record)
    });
    ports.seed_stored(vec![record]);
    resurrected(&ports).await;
    assert!(
        ports
            .signal_scopes()
            .iter()
            .all(|(_, scope)| *scope == SignalScope::ProcessGroup),
        "got: {:?}",
        ports.signal_scopes()
    );
}
