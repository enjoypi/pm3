use entities::{ProcessStatus, SandboxMode};

use super::test_helpers::*;

#[test]
fn view_projects_identity_and_spec_fields() {
    let candidate = record("api", 3);
    let view = candidate.view(5000);
    assert_eq!(view.pm_id, 3);
    assert_eq!(view.name, "api");
    assert_eq!(view.script, "/usr/bin/true");
    assert_eq!(view.cwd, "/srv/app");
    assert_eq!(view.sandbox_mode, SandboxMode::WorkspaceWrite.as_str());
    assert!(!view.sandbox_network);
}

#[test]
fn view_of_a_stopped_record_has_no_uptime() {
    let view = record("api", 1).view(5000);
    assert_eq!(view.status, ProcessStatus::Stopped);
    assert_eq!(view.uptime_ms, None);
    assert_eq!(view.pid, None);
}

#[test]
fn view_of_a_running_record_reports_uptime() {
    let mut candidate = record("api", 1);
    candidate.runtime.mark_launched(77, 2000);
    candidate.runtime.mark_online();
    let view = candidate.view(5000);
    assert_eq!(view.pid, Some(77));
    assert_eq!(view.uptime_ms, Some(3000));
    assert_eq!(view.status, ProcessStatus::Online);
}

#[test]
fn view_carries_dependencies_and_writable_roots() {
    let mut candidate = record("api", 1);
    candidate.spec.depends_on = vec!["db".to_string()];
    candidate.spec.sandbox.writable_roots = vec!["/var/cache/api".to_string()];
    candidate.spec.args = vec!["--port".to_string(), "8080".to_string()];
    let view = candidate.view(5000);
    assert_eq!(view.depends_on, ["db".to_string()]);
    assert_eq!(view.writable_roots, ["/var/cache/api".to_string()]);
    assert_eq!(view.args, ["--port".to_string(), "8080".to_string()]);
    assert_eq!(view.restart_time, 0);
}
