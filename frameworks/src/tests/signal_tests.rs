use std::time::Duration;

use super::*;

const HANDLER_SETTLE: Duration = Duration::from_millis(200);

async fn signal_self(name: &str) {
    let pid = std::process::id().to_string();
    let status = tokio::process::Command::new("/bin/kill")
        .args([name, &pid])
        .status()
        .await
        .expect("should signal this process");
    assert!(status.success(), "kill {name} should succeed");
}

#[tokio::test]
async fn sigint_is_swallowed_and_only_sigterm_stops_the_daemon() {
    let waiting = tokio::spawn(daemon_shutdown_signal());
    tokio::time::sleep(HANDLER_SETTLE).await;

    signal_self("-INT").await;
    tokio::time::sleep(HANDLER_SETTLE).await;
    assert!(!waiting.is_finished(), "SIGINT must not stop the daemon");

    signal_self("-TERM").await;
    tokio::time::timeout(Duration::from_secs(5), waiting)
        .await
        .expect("SIGTERM should stop the daemon")
        .expect("join");
}
