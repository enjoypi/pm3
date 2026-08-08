use super::*;

#[tokio::test]
async fn a_disabled_rotation_leaves_logs_alone_but_rearms() {
    let mut harness = harness_with_log_rotate(0, 20);
    std::fs::write(harness.paths.logs_dir.join("web-out.log"), vec![b'x'; 4096])
        .expect("seed the log");

    harness.daemon.on_log_rotate().await;

    assert_eq!(
        std::fs::metadata(harness.paths.logs_dir.join("web-out.log"))
            .expect("stat")
            .len(),
        4096
    );
    let event = next_event(&mut harness.events).await;
    assert!(
        matches!(event, DaemonEvent::RotateLogs),
        "the rotator must keep itself running, got: {event:?}"
    );
}

#[tokio::test]
async fn an_oversized_log_is_rotated_and_the_tick_rearms() {
    let mut harness = harness_with_log_rotate(1024, 20);
    std::fs::write(harness.paths.logs_dir.join("web-out.log"), vec![b'x'; 4096])
        .expect("seed the log");

    harness.daemon.on_log_rotate().await;

    assert_eq!(
        std::fs::metadata(harness.paths.logs_dir.join("web-out.log"))
            .expect("stat")
            .len(),
        0
    );
    assert!(
        harness.paths.logs_dir.join("web-out.log.1").is_file(),
        "the backup should exist"
    );
    let event = next_event(&mut harness.events).await;
    assert!(matches!(event, DaemonEvent::RotateLogs), "got: {event:?}");
}

#[tokio::test]
async fn a_missing_logs_directory_only_warns_and_rearms() {
    let mut harness = harness_with_log_rotate(1024, 20);
    std::fs::remove_dir(&harness.paths.logs_dir).expect("remove the log directory");

    harness.daemon.on_log_rotate().await;

    let event = next_event(&mut harness.events).await;
    assert!(
        matches!(event, DaemonEvent::RotateLogs),
        "a scan failure must not kill the tick, got: {event:?}"
    );
}
