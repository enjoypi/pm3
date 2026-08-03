use super::*;

fn stranded(ports: &FakePorts) -> StrandedProcess {
    ports.seed_live(SURVIVOR_PID, &live_token(SURVIVOR_PID));
    StrandedProcess {
        name: "api".to_string(),
        pid: Some(SURVIVOR_PID),
        token: Some(live_token(SURVIVOR_PID)),
    }
}

#[tokio::test]
async fn a_survivor_pm3_can_no_longer_manage_is_stopped() {
    let ports = FakePorts::new(1000);
    ports.seed_stranded(vec![stranded(&ports)]);
    resurrected(&ports).await;
    assert_eq!(
        ports.terminated(),
        vec![SURVIVOR_PID],
        "an unmanageable survivor must not outlive the daemon that lost its declaration"
    );
}

#[tokio::test]
async fn a_survivor_pm3_can_no_longer_manage_is_never_revived() {
    let ports = FakePorts::new(1000);
    ports.seed_stranded(vec![stranded(&ports)]);
    let table = resurrected(&ports).await;
    assert!(ports.spawned_names().is_empty());
    assert!(table.find(&AppSelector::Name("api".to_string())).is_none());
}

#[tokio::test]
async fn a_stranded_pid_that_already_left_is_not_signalled() {
    let ports = FakePorts::new(1000);
    let orphan = stranded(&ports);
    ports.hide_from_probe(SURVIVOR_PID);
    ports.seed_stranded(vec![orphan]);
    resurrected(&ports).await;
    assert!(ports.terminated().is_empty());
}

#[tokio::test]
async fn a_stranded_pid_the_kernel_reused_is_spared() {
    let ports = FakePorts::new(1000);
    let orphan = stranded(&ports);
    ports.seed_live(SURVIVOR_PID, "some other process");
    ports.seed_stranded(vec![orphan]);
    resurrected(&ports).await;
    assert!(ports.terminated().is_empty());
}

#[tokio::test]
async fn a_stranded_record_without_a_pid_signals_nothing() {
    let ports = FakePorts::new(1000);
    ports.seed_stranded(vec![StrandedProcess {
        name: "api".to_string(),
        pid: None,
        token: None,
    }]);
    resurrected(&ports).await;
    assert!(ports.terminated().is_empty());
}

#[tokio::test]
async fn a_stubborn_stranded_survivor_is_force_killed() {
    let ports = FakePorts::new(1000);
    ports.make_stubborn(SURVIVOR_PID);
    ports.seed_stranded(vec![stranded(&ports)]);
    resurrected(&ports).await;
    assert_eq!(ports.force_killed(), vec![SURVIVOR_PID]);
}
