use std::{io, time::Duration};

use super::*;

const HANDLER_SETTLE: Duration = Duration::from_millis(200);

#[cfg(unix)]
async fn signal_self(name: &str) {
    let pid = std::process::id().to_string();
    let status = tokio::process::Command::new("/bin/kill")
        .args([name, &pid])
        .status()
        .await
        .expect("should signal this process");
    assert!(status.success(), "kill {name} should succeed");
}

#[cfg(unix)]
#[tokio::test]
async fn sigint_is_swallowed_and_only_sigterm_stops_the_daemon() {
    let signals = ShutdownSignals::register().expect("register the shutdown handlers");
    let waiting = tokio::spawn(signals.wait());
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

#[cfg(unix)]
#[tokio::test]
async fn a_failing_interrupt_registration_is_reported() {
    let refuse = |_kind: SignalKind| -> io::Result<Signal> { Err(io::Error::other("no free fds")) };
    let err = ShutdownSignals::register_with(&refuse)
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("cannot register the SIGINT handler"),
        "got: {err}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_failing_terminate_registration_is_reported() {
    let calls = std::cell::Cell::new(0_u32);
    let refuse_second = |kind: SignalKind| {
        let call = calls.get();
        calls.set(call + 1);
        if call == 0 {
            return signal(kind);
        }
        Err(io::Error::other("no free fds"))
    };
    let err = ShutdownSignals::register_with(&refuse_second)
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("cannot register the SIGTERM handler"),
        "got: {err}"
    );
}

#[cfg(windows)]
#[tokio::test]
async fn a_failing_ctrl_c_registration_is_reported() {
    let refuse = || -> io::Result<CtrlC> { Err(io::Error::other("no console")) };
    let err = ShutdownSignals::register_with(&refuse, &ctrl_shutdown)
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("cannot register the CTRL_C handler"),
        "got: {err}"
    );
}

#[cfg(windows)]
#[tokio::test]
async fn a_failing_ctrl_shutdown_registration_is_reported() {
    let refuse = || -> io::Result<CtrlShutdown> { Err(io::Error::other("no console")) };
    let err = ShutdownSignals::register_with(&ctrl_c, &refuse)
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("cannot register the CTRL_SHUTDOWN handler"),
        "got: {err}"
    );
}

#[cfg(windows)]
#[tokio::test]
async fn registered_handlers_keep_the_daemon_waiting_until_an_event_arrives() {
    let signals = ShutdownSignals::register().expect("register the shutdown handlers");
    let waiting = tokio::spawn(signals.wait());
    tokio::time::sleep(HANDLER_SETTLE).await;
    assert!(
        !waiting.is_finished(),
        "no console event arrived, so the daemon must keep running"
    );
    waiting.abort();
}
