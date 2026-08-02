use super::*;

#[test]
fn a_daemon_that_already_left_is_reported_on_its_own() {
    assert_eq!(render_daemon_gone(None), DAEMON_NOT_RUNNING);
}

#[test]
fn a_daemon_that_already_left_keeps_the_stop_all_report_above_it() {
    let report = render_daemon_gone(Some("stopped all"));
    assert_eq!(report, format!("stopped all\n{DAEMON_NOT_RUNNING}"));
}

#[test]
fn a_stopped_daemon_warns_that_the_services_outlive_it() {
    let report = render_daemon_stopped(None, 42);
    assert_eq!(
        report,
        "stopped the pm3 daemon (pid 42); managed services keep running"
    );
}

#[test]
fn a_stopped_daemon_that_took_its_services_reports_both() {
    let report = render_daemon_stopped(Some("stopped web"), 42);
    assert_eq!(report, "stopped web\nstopped the pm3 daemon (pid 42)");
}
