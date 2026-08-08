use super::*;
use crate::process_views::running_view;

#[test]
fn the_dto_mirrors_the_view_field_by_field() {
    let dto = ProcessViewDto::from(&running_view(7, "web"));
    assert_eq!(dto.pm_id, 7);
    assert_eq!(dto.name, "web");
    assert_eq!(dto.pid, Some(crate::process_views::RUNNING_PID));
    assert_eq!(dto.status, "online");
    assert_eq!(dto.restart_time, 2);
    assert_eq!(dto.uptime_ms, Some(5000));
    assert_eq!(dto.next_fire_ms, None);
    assert_eq!(dto.schedule, None);
    assert_eq!(dto.sandbox_mode, "workspace-write");
    assert!(!dto.sandbox_network);
    assert_eq!(dto.script, "/usr/bin/node");
    assert_eq!(dto.args, ["server.js", "--port=8080"]);
    assert_eq!(dto.cwd, "/srv/web");
    assert_eq!(dto.depends_on, ["db"]);
    assert_eq!(dto.writable_roots, ["/srv/web"]);
    assert_eq!(dto.rss_kib, None);
    assert_eq!(dto.cpu_tenths, None);
}

#[test]
fn the_serialized_dto_has_no_env_field() {
    let dto = ProcessViewDto::from(&running_view(0, "web"));
    let json = serde_json::to_string(&dto).expect("serialize");
    assert!(!json.contains("env"), "got: {json}");
    assert!(json.contains("\"name\":\"web\""), "got: {json}");
}
