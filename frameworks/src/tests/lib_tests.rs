use super::*;

#[test]
fn a_config_failure_is_passed_through() {
    let inner = adapters::load_and_parse_config("/nonexistent/pm3.yaml").unwrap_err();
    let expected = inner.to_string();
    assert_eq!(Error::from(inner).to_string(), expected);
}

#[test]
fn a_path_failure_is_passed_through() {
    let inner = adapters::expand_home("~/.pm3", None).unwrap_err();
    let expected = inner.to_string();
    assert_eq!(Error::from(inner).to_string(), expected);
}

#[test]
fn a_log_failure_is_passed_through() {
    let inner = adapters::LogReadError {
        path: "/tmp/web-out.log".to_string(),
        reason: "gone".to_string(),
    };
    let expected = inner.to_string();
    assert_eq!(Error::from(inner).to_string(), expected);
}

#[test]
fn a_telemetry_failure_is_passed_through() {
    let inner = telemetry::TelemetryError::InvalidFilter("bad".to_string());
    let expected = inner.to_string();
    assert_eq!(Error::from(inner).to_string(), expected);
}

#[test]
fn a_serve_failure_is_passed_through() {
    let inner = server::ServerError::Serve(std::io::Error::other("boom"));
    let expected = inner.to_string();
    assert_eq!(Error::from(inner).to_string(), expected);
}

#[test]
fn a_socket_failure_is_passed_through() {
    let inner = daemon::SocketError::Bind {
        path: "/tmp/pm3.sock".to_string(),
        reason: "busy".to_string(),
    };
    let expected = inner.to_string();
    assert_eq!(Error::from(inner).to_string(), expected);
}

#[test]
fn a_client_failure_is_passed_through() {
    let inner = client::ClientError::Silent {
        path: "/tmp/pm3.sock".to_string(),
    };
    let expected = inner.to_string();
    assert_eq!(Error::from(inner).to_string(), expected);
}

#[test]
fn a_layout_failure_names_the_home() {
    let error = Error::Layout {
        path: "/srv/pm3".to_string(),
        reason: "read-only".to_string(),
    };
    assert_eq!(
        error.to_string(),
        "cannot prepare the pm3 home '/srv/pm3': read-only"
    );
}

#[test]
fn an_apps_file_failure_names_the_file() {
    let error = Error::AppsFile {
        path: "apps.yaml".to_string(),
        reason: "missing".to_string(),
    };
    assert_eq!(
        error.to_string(),
        "cannot resolve the apps file 'apps.yaml': missing"
    );
}

#[test]
fn a_spawn_failure_explains_itself() {
    let error = Error::DaemonSpawn {
        reason: "no such file".to_string(),
    };
    assert_eq!(
        error.to_string(),
        "cannot spawn the pm3 daemon: no such file"
    );
}

#[test]
fn an_unready_daemon_reports_the_budget() {
    let error = Error::DaemonUnready {
        path: "/tmp/pm3.sock".to_string(),
        timeout_ms: 400,
    };
    assert_eq!(
        error.to_string(),
        "cannot reach the pm3 daemon on '/tmp/pm3.sock' within 400 ms"
    );
}

#[test]
fn a_service_manager_failure_is_passed_through() {
    let error = Error::Service(adapters::ServiceCommandError::Failed {
        program: "/bin/launchctl".to_string(),
        reason: "exited with status 1".to_string(),
    });
    assert_eq!(
        error.to_string(),
        "cannot complete '/bin/launchctl': exited with status 1"
    );
}

#[test]
fn a_binary_lookup_failure_explains_itself() {
    let error = Error::ServiceProgram {
        reason: "no such process image".to_string(),
    };
    assert_eq!(
        error.to_string(),
        "cannot determine the pm3 binary path: no such process image"
    );
}

#[test]
fn an_unresolvable_config_path_names_the_path() {
    let error = Error::ServiceConfig {
        path: "config.yaml".to_string(),
        reason: "missing".to_string(),
    };
    assert_eq!(
        error.to_string(),
        "cannot resolve the config path 'config.yaml': missing"
    );
}

#[test]
fn a_missing_home_explains_itself() {
    assert_eq!(
        Error::ServiceHome.to_string(),
        "cannot locate the service directory: no HOME in the environment"
    );
}

#[test]
fn a_refused_request_reports_the_status_and_the_body() {
    let error = Error::Refused {
        status: 404,
        body: "cannot find app 'web'".to_string(),
    };
    assert_eq!(
        error.to_string(),
        "pm3 daemon refused the request with status 404: cannot find app 'web'"
    );
}
