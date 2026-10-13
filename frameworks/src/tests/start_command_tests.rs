use super::*;

fn running_web_report() -> commands::StartReport {
    commands::StartReport {
        response: "web is already running (id 0, pid 1)".to_string(),
        changed: vec!["web".to_string()],
        already_running: vec!["web".to_string()],
    }
}

#[tokio::test]
async fn an_accepted_restart_runs_and_reports() {
    let fixture = crate::daemon_fixture::running_daemon().await;
    let apps_file = crate::daemon_fixture::sleeper_apps_file(&fixture);
    commands::start_apps(&fixture.config_path, &apps_file, false)
        .await
        .expect("should start");
    let offered = offer_restarts(&fixture.config_path, &running_web_report(), &mut |_| true)
        .await
        .expect("should offer");
    assert_eq!(offered.as_deref(), Some("restarted web"));
    crate::daemon_fixture::stop_daemon(fixture).await;
}

#[tokio::test]
async fn a_declined_restart_reports_the_hint() {
    let offered = offer_restarts("/nonexistent/pm3.yaml", &running_web_report(), &mut |_| {
        false
    })
    .await
    .expect("should offer");
    assert_eq!(
        offered.as_deref(),
        Some(
            "'web' keeps running with the previous config; run 'pm3 restart web' to apply the new one"
        )
    );
}

#[tokio::test]
async fn a_failing_restart_aborts_the_offer() {
    let outcome = offer_restarts("/nonexistent/pm3.yaml", &running_web_report(), &mut |_| {
        true
    })
    .await;
    assert!(outcome.is_err(), "got: {outcome:?}");
}

#[tokio::test]
async fn nothing_changed_offers_nothing() {
    let report = commands::StartReport {
        response: "started web (id 0, pid 1)".to_string(),
        changed: Vec::new(),
        already_running: Vec::new(),
    };
    let offered = offer_restarts("/nonexistent/pm3.yaml", &report, &mut |_| true)
        .await
        .expect("should offer");
    assert_eq!(offered, None);
}
