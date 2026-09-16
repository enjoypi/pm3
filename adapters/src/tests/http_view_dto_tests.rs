use super::*;
use crate::process_views::running_view;

#[test]
fn the_dto_mirrors_the_view_field_by_field() {
    let dto = ProcessViewDto::from(&running_view(7, "web"));
    assert_eq!(dto.pm_id, 7);
    assert_eq!(dto.name, "web");
    assert_eq!(dto.pid, Some(crate::process_views::RUNNING_PID));
    assert_eq!(dto.status, "online");
    assert_eq!(dto.restart_time, crate::process_views::RESTART_TIME);
    assert_eq!(
        dto.unstable_restarts,
        crate::process_views::UNSTABLE_RESTARTS
    );
    assert_eq!(dto.max_restarts, crate::process_views::MAX_RESTARTS);
    assert!(dto.autorestart);
    assert_eq!(dto.sandbox_read, "minimal");
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
    assert!(!json.contains("\"env\":"), "got: {json}");
    assert!(
        json.contains("\"env_origin\":\"plain\""),
        "the origin is a label, never a value: {json}"
    );
    assert!(json.contains("\"name\":\"web\""), "got: {json}");
}

#[test]
fn the_dto_carries_every_environment_origin() {
    for (origin, shown) in [
        (usecases::EnvOrigin::Plain, "plain"),
        (usecases::EnvOrigin::Encrypted, "encrypted"),
        (usecases::EnvOrigin::Sealed, "sealed"),
    ] {
        let view = usecases::ProcessView {
            env_origin: origin,
            ..running_view(0, "web")
        };
        assert_eq!(ProcessViewDto::from(&view).env_origin, shown);
    }
}

#[test]
fn the_dto_carries_how_many_values_were_declared() {
    let view = usecases::ProcessView {
        env_declared: 3,
        ..running_view(0, "web")
    };
    assert_eq!(ProcessViewDto::from(&view).env_declared, 3);
}

#[test]
fn the_serialized_dto_carries_no_cleartext_value() {
    let secret = "abcdefghijklmnopqrstuvwxyz";
    let view = crate::process_views::view_with_env(
        0,
        "web",
        vec![usecases::EnvDisplay {
            key: "CF_API_TOKEN".to_string(),
            value: usecases::mask_secret(secret),
            scope: usecases::EnvScope::App,
        }],
    );
    let json = serde_json::to_string(&ProcessViewDto::from(&view)).expect("serialize");
    assert!(
        !json.contains(secret),
        "the middle of a value never leaves the daemon: {json}"
    );
    assert!(json.contains("abcd..wxyz 26"), "got: {json}");
    assert!(!json.contains("\"env\":"), "got: {json}");
}

#[test]
fn an_app_without_values_omits_the_environment_field() {
    let dto = ProcessViewDto::from(&running_view(0, "web"));
    let json = serde_json::to_string(&dto).expect("serialize");
    assert!(!json.contains("env_masked"), "got: {json}");
}
