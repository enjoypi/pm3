use entities::AppSpec;

use super::*;
use crate::{
    AppSelector, UsecaseError,
    ports_test_helpers::{FakePorts, LOGS_DIR, spec},
    start::start_apps,
};

const CRASH: ExitOutcome = ExitOutcome { exit_code: Some(1) };
const CLEAN: ExitOutcome = ExitOutcome { exit_code: Some(0) };

async fn running_table(ports: &FakePorts, candidate: AppSpec) -> ProcessTable {
    let mut table = ProcessTable::new();
    start_apps(&mut table, &[candidate], LOGS_DIR, ports)
        .await
        .expect("start should succeed");
    table
}

#[tokio::test]
async fn a_stable_crash_schedules_a_restart_after_the_configured_delay() {
    let ports = FakePorts::new(1000);
    let mut table = running_table(&ports, spec("api")).await;
    ports.advance_to(9000);
    let action = handle_child_exit(&mut table, "api", CRASH, &ports)
        .await
        .expect("exit handled");
    assert_eq!(action, ExitAction::RestartAfter { delay_ms: 250 });
    let record = table.find(&AppSelector::Id(0)).expect("record present");
    assert_eq!(record.runtime.restart_time, 1);
    assert_eq!(record.runtime.unstable_restarts, 0);
}

#[tokio::test]
async fn repeated_fast_crashes_trip_the_breaker_into_errored() {
    let ports = FakePorts::new(1000);
    let candidate = AppSpec {
        max_restarts: 2,
        ..spec("api")
    };
    let mut table = running_table(&ports, candidate).await;

    let first = handle_child_exit(&mut table, "api", CRASH, &ports)
        .await
        .expect("exit handled");
    assert_eq!(first, ExitAction::RestartAfter { delay_ms: 250 });

    let relaunched = table.find_mut(&AppSelector::Id(0)).expect("record present");
    relaunched.runtime.mark_launched(101, 1000);
    relaunched.runtime.mark_online();

    let second = handle_child_exit(&mut table, "api", CRASH, &ports)
        .await
        .expect("exit handled");
    assert_eq!(
        second,
        ExitAction::Settled {
            status: ProcessStatus::Errored,
        }
    );
    let record = table.find(&AppSelector::Id(0)).expect("record present");
    assert_eq!(record.runtime.status, ProcessStatus::Errored);
    assert_eq!(record.runtime.unstable_restarts, 2);
}

#[tokio::test]
async fn a_clean_exit_without_autorestart_settles_as_stopped() {
    let ports = FakePorts::new(1000);
    let candidate = AppSpec {
        autorestart: false,
        ..spec("api")
    };
    let mut table = running_table(&ports, candidate).await;
    let action = handle_child_exit(&mut table, "api", CLEAN, &ports)
        .await
        .expect("exit handled");
    assert_eq!(
        action,
        ExitAction::Settled {
            status: ProcessStatus::Stopped,
        }
    );
}

#[tokio::test]
async fn a_crash_without_autorestart_settles_as_errored() {
    let ports = FakePorts::new(1000);
    let candidate = AppSpec {
        autorestart: false,
        ..spec("api")
    };
    let mut table = running_table(&ports, candidate).await;
    let action = handle_child_exit(&mut table, "api", CRASH, &ports)
        .await
        .expect("exit handled");
    assert_eq!(
        action,
        ExitAction::Settled {
            status: ProcessStatus::Errored,
        }
    );
}

#[tokio::test]
async fn an_operator_stop_settles_without_restarting() {
    let ports = FakePorts::new(1000);
    let mut table = running_table(&ports, spec("api")).await;
    let stopping = table.find_mut(&AppSelector::Id(0)).expect("record present");
    stopping.runtime.mark_stopping();
    let action = handle_child_exit(&mut table, "api", CRASH, &ports)
        .await
        .expect("exit handled");
    assert_eq!(
        action,
        ExitAction::Settled {
            status: ProcessStatus::Stopped,
        }
    );
    let record = table.find(&AppSelector::Id(0)).expect("record present");
    assert_eq!(record.runtime.restart_time, 0);
}

#[tokio::test]
async fn an_operator_restart_reschedules_without_delay() {
    let ports = FakePorts::new(1000);
    let mut table = running_table(&ports, spec("api")).await;
    let restarting = table.find_mut(&AppSelector::Id(0)).expect("record present");
    restarting.runtime.mark_stopping();
    restarting.runtime.request_restart();
    let action = handle_child_exit(&mut table, "api", CRASH, &ports)
        .await
        .expect("exit handled");
    assert_eq!(action, ExitAction::RestartAfter { delay_ms: 0 });
    let record = table.find(&AppSelector::Id(0)).expect("record present");
    assert!(!record.runtime.pending_restart);
}

#[tokio::test]
async fn an_exit_for_an_unknown_app_reports_not_found() {
    let ports = FakePorts::new(1000);
    let mut table = ProcessTable::new();
    let err = handle_child_exit(&mut table, "ghost", CRASH, &ports)
        .await
        .unwrap_err();
    assert!(matches!(err, UsecaseError::NotFound(_)), "got: {err}");
}

#[tokio::test]
async fn a_persistence_failure_propagates() {
    let ports = FakePorts::new(1000);
    let mut table = running_table(&ports, spec("api")).await;
    ports.fail_save();
    let err = handle_child_exit(&mut table, "api", CRASH, &ports)
        .await
        .unwrap_err();
    assert!(matches!(err, UsecaseError::Dump(_)), "got: {err}");
}

#[tokio::test]
async fn handling_an_exit_persists_the_table() {
    let ports = FakePorts::new(1000);
    let mut table = running_table(&ports, spec("api")).await;
    handle_child_exit(&mut table, "api", CRASH, &ports)
        .await
        .expect("exit handled");
    assert_eq!(ports.save_count(), 2);
}
