use super::*;

#[test]
fn a_start_request_carries_the_service_names() {
    assert_eq!(
        encode_start_request(&["web".to_string()]),
        "{\"services\":[\"web\"]}"
    );
}

#[test]
fn a_start_request_without_services_stays_an_empty_list() {
    assert_eq!(encode_start_request(&[]), "{\"services\":[]}");
}

#[test]
fn a_reply_body_decodes_into_its_envelope() {
    let reply = decode_reply("{\"report\":\"started web\"}").expect("should decode");
    assert_eq!(reply.report, "started web");
}

#[test]
fn a_reply_body_that_is_not_json_is_refused() {
    let err = decode_reply("<html>bad gateway</html>")
        .unwrap_err()
        .to_string();
    assert!(
        err.starts_with("cannot decode the pm3 daemon reply"),
        "got: {err}"
    );
}

#[test]
fn a_named_app_addresses_its_own_route() {
    assert_eq!(app_path("web").expect("should address"), "/apps/web");
}

#[test]
fn a_pm_id_addresses_a_route_without_a_name_check() {
    assert_eq!(app_path("3").expect("should address"), "/apps/3");
}

#[test]
fn an_unsafe_name_cannot_address_a_route() {
    let err = app_path("../../.bashrc").unwrap_err().to_string();
    assert!(err.contains("../../.bashrc"), "got: {err}");
}

#[test]
fn an_action_route_carries_the_action_after_the_selector() {
    assert_eq!(
        app_action_path("web", "restart").expect("should address"),
        "/apps/web/restart"
    );
}

#[test]
fn an_unsafe_name_cannot_address_an_action_route() {
    let err = app_action_path("my app", "stop").unwrap_err().to_string();
    assert!(err.contains("my app"), "got: {err}");
}
