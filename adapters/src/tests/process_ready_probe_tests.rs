use super::*;

const TIMEOUT_MS: u64 = 500;
const SEARCH_PATH: &str = "/usr/bin:/bin";

fn exec_probe(command: &[&str]) -> ReadyProbe {
    ReadyProbe::Exec {
        command: command.iter().map(ToString::to_string).collect(),
    }
}

#[tokio::test]
async fn an_exec_probe_passes_when_the_command_succeeds() {
    let prober = HostReadyProber::new(TIMEOUT_MS, SEARCH_PATH.to_string());
    let probe = exec_probe(&["/usr/bin/true"]);
    assert_eq!(prober.check_ready(&probe).await, Readiness::Ready);
}

#[tokio::test]
async fn an_exec_probe_stays_pending_when_the_command_fails() {
    let prober = HostReadyProber::new(TIMEOUT_MS, SEARCH_PATH.to_string());
    let probe = exec_probe(&["/usr/bin/false"]);
    assert_eq!(prober.check_ready(&probe).await, Readiness::Pending);
}

#[tokio::test]
async fn an_exec_probe_fails_fast_when_the_command_is_missing() {
    let prober = HostReadyProber::new(TIMEOUT_MS, SEARCH_PATH.to_string());
    let probe = exec_probe(&["/nonexistent/probe"]);
    let outcome = prober.check_ready(&probe).await;
    assert!(matches!(outcome, Readiness::Failed(_)), "got: {outcome:?}");
}

#[tokio::test]
async fn an_exec_probe_without_a_command_fails_fast() {
    let prober = HostReadyProber::new(TIMEOUT_MS, SEARCH_PATH.to_string());
    let probe = exec_probe(&[]);
    let outcome = prober.check_ready(&probe).await;
    assert!(matches!(outcome, Readiness::Failed(_)), "got: {outcome:?}");
}

#[tokio::test]
async fn an_exec_probe_stays_pending_when_the_command_overruns() {
    let prober = HostReadyProber::new(30, SEARCH_PATH.to_string());
    let probe = exec_probe(&["/bin/sleep", "5"]);
    assert_eq!(prober.check_ready(&probe).await, Readiness::Pending);
}

#[cfg(unix)]
#[tokio::test]
async fn an_exec_probe_finds_a_bare_program_on_the_search_path() {
    use std::os::unix::fs::PermissionsExt as _;

    let dir = tempfile::tempdir().expect("temp dir");
    let program = dir.path().join("probe-ok");
    std::fs::write(&program, "#!/bin/sh\nexit 0\n").expect("write the probe program");
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755))
        .expect("make the probe program executable");
    let prober = HostReadyProber::new(5_000, dir.path().to_string_lossy().into_owned());
    let probe = exec_probe(&["probe-ok"]);
    assert_eq!(prober.check_ready(&probe).await, Readiness::Ready);
}

#[cfg(unix)]
#[tokio::test]
async fn an_exec_probe_fails_fast_when_the_program_cannot_start() {
    use std::os::unix::fs::PermissionsExt as _;

    let dir = tempfile::tempdir().expect("temp dir");
    let program = dir.path().join("probe-broken");
    std::fs::write(&program, "#!/nonexistent/interpreter\n").expect("write the probe program");
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755))
        .expect("make the probe program executable");
    let prober = HostReadyProber::new(TIMEOUT_MS, dir.path().to_string_lossy().into_owned());
    let probe = exec_probe(&["probe-broken"]);
    let outcome = prober.check_ready(&probe).await;
    assert!(matches!(outcome, Readiness::Failed(_)), "got: {outcome:?}");
}

#[tokio::test]
async fn an_exec_probe_reports_a_bare_program_missing_from_the_search_path() {
    let prober = HostReadyProber::new(TIMEOUT_MS, "/nonexistent".to_string());
    let probe = exec_probe(&["true"]);
    let outcome = prober.check_ready(&probe).await;
    assert!(matches!(outcome, Readiness::Failed(_)), "got: {outcome:?}");
}

#[tokio::test]
async fn a_tcp_probe_passes_when_the_port_answers() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind a throwaway listener");
    let port = listener
        .local_addr()
        .expect("the listener has an address")
        .port();
    let prober = HostReadyProber::new(TIMEOUT_MS, SEARCH_PATH.to_string());
    let probe = ReadyProbe::Tcp {
        host: "127.0.0.1".to_string(),
        port,
    };
    assert_eq!(prober.check_ready(&probe).await, Readiness::Ready);
}

#[tokio::test]
async fn a_tcp_probe_stays_pending_when_the_port_refuses() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind a throwaway listener");
    let port = listener
        .local_addr()
        .expect("the listener has an address")
        .port();
    drop(listener);
    let prober = HostReadyProber::new(TIMEOUT_MS, SEARCH_PATH.to_string());
    let probe = ReadyProbe::Tcp {
        host: "127.0.0.1".to_string(),
        port,
    };
    assert_eq!(prober.check_ready(&probe).await, Readiness::Pending);
}
