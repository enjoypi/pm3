use std::path::{Path, PathBuf};

use usecases::AlertKind;

use super::*;
use crate::platform::{SEARCH_PATH, script};

const TIMEOUT_MS: u64 = 5000;

fn alert() -> Alert {
    Alert {
        name: "api".to_string(),
        kind: AlertKind::LaunchFailed,
    }
}

fn notifier(program: &Path, enabled: bool, timeout_ms: u64) -> DesktopNotifier {
    let config = NotifyConfig {
        enabled,
        notify_send_program: program.to_string_lossy().into_owned(),
        ..NotifyConfig::default()
    };
    DesktopNotifier::new(
        &config,
        &[NotifyBackend::NotifySend],
        timeout_ms,
        SEARCH_PATH.to_string(),
    )
}

fn recorder(dir: &Path, exit: &str) -> (PathBuf, PathBuf) {
    let out = dir.join("argv");
    let unix = format!("printf '%s\\n' \"$@\" > '{}'\n{exit}", out.display());
    let windows = format!("echo %* > \"{}\"\r\n{exit}", out.display());
    (script(dir, "notify", &unix, &windows), out)
}

#[test]
fn osascript_receives_the_text_as_arguments_not_as_script() {
    let argv = NotifyBackend::Osascript.argv("pm3: \"api\"", "a \"quoted\" body");

    assert_eq!(argv[..6], OSASCRIPT_SCRIPT);
    assert_eq!(argv[6..], ["pm3: \"api\"", "a \"quoted\" body"]);
}

#[test]
fn notify_send_takes_only_title_and_body() {
    assert_eq!(
        NotifyBackend::NotifySend.argv("title", "body"),
        ["title", "body"]
    );
}

#[tokio::test]
async fn an_alert_runs_the_backend_with_title_and_body() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (program, out) = recorder(dir.path(), "exit 0");

    notifier(&program, true, TIMEOUT_MS).notify(&alert()).await;

    let written = std::fs::read_to_string(out).expect("the backend should run");
    assert!(written.contains("pm3: api"), "got: {written}");
    assert!(written.contains("自动启动失败"), "got: {written}");
}

#[tokio::test]
async fn a_disabled_notifier_runs_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (program, out) = recorder(dir.path(), "exit 0");

    notifier(&program, false, TIMEOUT_MS).notify(&alert()).await;

    assert!(!out.exists(), "a disabled notifier must stay silent");
}

#[tokio::test]
async fn a_failing_backend_is_tolerated() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (program, out) = recorder(dir.path(), "exit 1");

    notifier(&program, true, TIMEOUT_MS).notify(&alert()).await;

    assert!(out.exists(), "the backend should still have run");
}

#[tokio::test]
async fn a_stalled_backend_is_abandoned() {
    let dir = tempfile::tempdir().expect("tempdir");
    let program = script(dir.path(), "notify", "sleep 5", "ping -n 6 127.0.0.1 > nul");
    let started = std::time::Instant::now();

    notifier(&program, true, 100).notify(&alert()).await;

    assert!(
        started.elapsed().as_millis() < 4000,
        "the timeout must bound the wait"
    );
}

#[tokio::test]
async fn a_missing_backend_is_skipped() {
    let dir = tempfile::tempdir().expect("tempdir");

    notifier(&dir.path().join("absent"), true, TIMEOUT_MS)
        .notify(&alert())
        .await;
}

#[cfg(unix)]
#[tokio::test]
async fn a_backend_that_cannot_exec_is_skipped() {
    use std::os::unix::fs::PermissionsExt as _;

    let dir = tempfile::tempdir().expect("tempdir");
    let program = dir.path().join("broken");
    std::fs::write(&program, "#!/nonexistent/interpreter\n").expect("write the script");
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755))
        .expect("make the script executable");

    notifier(&program, true, TIMEOUT_MS).notify(&alert()).await;
}

#[tokio::test]
async fn the_osascript_backend_runs_its_own_program() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (program, out) = recorder(dir.path(), "exit 0");
    let config = NotifyConfig {
        osascript_program: program.to_string_lossy().into_owned(),
        ..NotifyConfig::default()
    };
    let notifier = DesktopNotifier::new(
        &config,
        &[NotifyBackend::Osascript],
        TIMEOUT_MS,
        SEARCH_PATH.to_string(),
    );

    notifier.notify(&alert()).await;

    let written = std::fs::read_to_string(out).expect("the backend should run");
    assert!(written.contains("on run argv"), "got: {written}");
}
