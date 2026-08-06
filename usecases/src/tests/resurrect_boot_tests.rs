use super::*;

const EARLIER_BOOT: &str = "Mon Jan 01 00:00:00 2020";
const CURRENT_BOOT: &str = "Tue Jul 28 14:06:28 2026";

fn ports_after_a_reboot() -> FakePorts {
    let ports = FakePorts::new(1000);
    ports.seed_boot(EARLIER_BOOT);
    ports.seed_live(INIT_PID, CURRENT_BOOT);
    ports
}

#[test]
fn a_state_file_from_this_very_boot_keeps_its_pids() {
    assert_eq!(
        PidTrust::of(Some(CURRENT_BOOT), Some(CURRENT_BOOT)),
        PidTrust::Kept
    );
}

#[test]
fn a_state_file_from_an_earlier_boot_loses_its_pids() {
    assert_eq!(
        PidTrust::of(Some(EARLIER_BOOT), Some(CURRENT_BOOT)),
        PidTrust::Lost
    );
}

#[test]
fn a_state_file_written_before_pm3_recorded_boots_keeps_its_pids() {
    assert_eq!(PidTrust::of(None, Some(CURRENT_BOOT)), PidTrust::Kept);
}

#[test]
fn a_host_that_cannot_report_its_boot_keeps_the_recorded_pids() {
    assert_eq!(PidTrust::of(Some(EARLIER_BOOT), None), PidTrust::Kept);
}

#[tokio::test]
async fn a_service_recorded_before_a_reboot_is_started_fresh() {
    let ports = ports_after_a_reboot();
    ports.seed_stored(vec![survivor(&ports, "api")]);
    resurrected(&ports).await;
    assert_eq!(ports.spawned_names(), vec!["api"]);
}

#[tokio::test]
async fn a_pid_recorded_before_a_reboot_is_never_signalled() {
    let ports = ports_after_a_reboot();
    ports.seed_stored(vec![survivor(&ports, "api")]);
    resurrected(&ports).await;
    assert!(
        ports.terminated().is_empty(),
        "whatever holds that pid now has nothing to do with pm3"
    );
}

#[tokio::test]
async fn a_service_caught_mid_shutdown_before_a_reboot_is_never_signalled() {
    let ports = ports_after_a_reboot();
    let mut record = survivor(&ports, "api");
    record.runtime.status = ProcessStatus::Stopping;
    ports.seed_stored(vec![record]);
    resurrected(&ports).await;
    assert!(ports.terminated().is_empty());
}

#[tokio::test]
async fn a_stranded_pid_recorded_before_a_reboot_is_never_signalled() {
    let ports = ports_after_a_reboot();
    ports.seed_stranded(vec![StrandedProcess {
        name: "api".to_string(),
        pid: Some(SURVIVOR_PID),
        token: Some(live_token(SURVIVOR_PID)),
    }]);
    ports.seed_live(SURVIVOR_PID, &live_token(SURVIVOR_PID));
    resurrected(&ports).await;
    assert!(ports.terminated().is_empty());
}

#[tokio::test]
async fn a_survivor_from_the_same_boot_is_still_reclaimed() {
    let ports = FakePorts::new(1000);
    ports.seed_boot(CURRENT_BOOT);
    ports.seed_live(INIT_PID, CURRENT_BOOT);
    ports.seed_stored(vec![survivor(&ports, "api")]);
    resurrected(&ports).await;
    assert_eq!(ports.adopted(), vec![SURVIVOR_PID]);
}

#[tokio::test]
async fn the_saved_state_carries_the_boot_it_was_written_under() {
    let ports = FakePorts::new(1000);
    ports.seed_live(INIT_PID, CURRENT_BOOT);
    resurrected(&ports).await;
    assert_eq!(ports.saved_boot(), Some(CURRENT_BOOT.to_string()));
}

#[tokio::test]
async fn a_host_that_cannot_report_its_boot_saves_no_boot_at_all() {
    let ports = FakePorts::new(1000);
    resurrected(&ports).await;
    assert_eq!(ports.saved_boot(), None);
}
