use super::*;
use crate::process_views::{RUNNING_PID, idle_view, running_view};

fn value_of(view: &ProcessView, label: &str) -> String {
    render_describe(view)
        .lines()
        .find_map(|line| line.strip_prefix(label).map(str::to_string))
        .map(|rest| rest.trim().to_string())
        .expect("labelled line")
}

#[test]
fn describe_reports_the_id() {
    assert_eq!(value_of(&running_view(7, "web"), "id"), "7");
}

#[test]
fn describe_reports_the_name() {
    assert_eq!(value_of(&running_view(0, "web"), "name"), "web");
}

#[test]
fn describe_reports_the_status() {
    assert_eq!(value_of(&running_view(0, "web"), "status"), "online");
}

#[test]
fn describe_reports_the_pid() {
    assert_eq!(
        value_of(&running_view(0, "web"), "pid"),
        RUNNING_PID.to_string()
    );
}

#[test]
fn describe_reports_the_uptime() {
    assert_eq!(value_of(&running_view(0, "web"), "uptime"), "5s");
}

#[test]
fn describe_reports_the_restart_count() {
    assert_eq!(value_of(&running_view(0, "web"), "restarts"), "2");
}

#[test]
fn describe_reports_the_script() {
    assert_eq!(value_of(&running_view(0, "web"), "script"), "/usr/bin/node");
}

#[test]
fn describe_reports_the_arguments() {
    assert_eq!(
        value_of(&running_view(0, "web"), "args"),
        "server.js, --port=8080"
    );
}

#[test]
fn describe_reports_the_working_directory() {
    assert_eq!(value_of(&running_view(0, "web"), "cwd"), "/srv/web");
}

#[test]
fn describe_reports_the_dependencies() {
    assert_eq!(value_of(&running_view(0, "web"), "depends on"), "db");
}

#[test]
fn describe_reports_the_sandbox() {
    assert_eq!(
        value_of(&running_view(0, "web"), "sandbox"),
        "workspace-write"
    );
}

#[test]
fn describe_reports_the_writable_roots() {
    assert_eq!(
        value_of(&running_view(0, "web"), "writable roots"),
        "/srv/web"
    );
}

#[test]
fn describe_marks_an_idle_app_as_having_no_pid() {
    assert_eq!(value_of(&idle_view(0, "web"), "pid"), "-");
}

#[test]
fn describe_marks_an_empty_list_as_missing() {
    assert_eq!(value_of(&idle_view(0, "web"), "args"), "-");
}

#[test]
fn every_field_gets_its_own_line() {
    assert_eq!(render_describe(&running_view(0, "web")).lines().count(), 14);
}

#[test]
fn labels_are_padded_to_the_longest_one() {
    let rendered = render_describe(&running_view(7, "web"));
    let gap = " ".repeat("writable roots".len() - "id".len() + LABEL_GAP.len());
    assert!(rendered.contains(&format!("id{gap}7")), "got: {rendered}");
}

#[test]
fn describe_reports_the_schedule_and_its_next_fire() {
    let mut view = idle_view(0, "sweep");
    view.schedule = Some("~ * * * *".to_string());
    view.next_fire_ms = Some(1_700_000_000_000);
    assert_eq!(value_of(&view, "schedule"), "~ * * * *");
    let stamp = value_of(&view, "next fire");
    assert!(stamp.contains("UTC+"), "stamp must name its zone: {stamp}");
}

#[test]
fn describe_marks_an_unscheduled_app_as_missing() {
    let view = idle_view(0, "web");
    assert_eq!(value_of(&view, "schedule"), "-");
    assert_eq!(value_of(&view, "next fire"), "-");
}
