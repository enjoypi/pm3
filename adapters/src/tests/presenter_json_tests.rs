use super::*;
use crate::process_views::running_view;

#[test]
fn a_list_of_views_serializes_as_a_json_array() {
    let views = [ProcessViewDto::from(&running_view(0, "web"))];
    let json = render_json_list(&views);
    assert!(json.starts_with('['), "got: {json}");
    assert!(json.contains("\"name\":\"web\""), "got: {json}");
}

#[test]
fn an_empty_list_serializes_as_an_empty_array() {
    assert_eq!(render_json_list(&[]), "[]");
}

#[test]
fn one_view_serializes_as_a_json_object() {
    let dto = ProcessViewDto::from(&running_view(0, "web"));
    let json = render_json_one(Some(&dto));
    assert!(json.starts_with('{'), "got: {json}");
}

#[test]
fn a_missing_view_serializes_as_null() {
    assert_eq!(render_json_one(None), "null");
}
