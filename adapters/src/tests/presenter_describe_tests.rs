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
    assert_eq!(
        value_of(&running_view(0, "web"), "restarts"),
        crate::process_views::RESTART_TIME.to_string()
    );
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
    assert_eq!(render_describe(&running_view(0, "web")).lines().count(), 18);
}

#[test]
fn labels_are_padded_to_the_longest_one() {
    let rendered = render_describe(&running_view(7, "web"));
    let gap = " ".repeat("unstable restarts".len() - "id".len() + LABEL_GAP.len());
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

#[test]
fn describe_reports_the_sampled_resources() {
    let mut view = running_view(0, "web");
    view.rss_kib = Some(1536);
    view.cpu_tenths = Some(7);
    assert_eq!(value_of(&view, "memory"), "1.5M");
    assert_eq!(value_of(&view, "cpu"), "0.7%");
}

#[test]
fn describe_marks_an_unsampled_app_as_missing() {
    let view = idle_view(0, "web");
    assert_eq!(value_of(&view, "memory"), "-");
    assert_eq!(value_of(&view, "cpu"), "-");
}

#[test]
fn describe_names_where_the_environment_came_from() {
    for (origin, shown) in [
        (usecases::EnvOrigin::Plain, "plain (2 values)"),
        (usecases::EnvOrigin::Encrypted, "encrypted (2 values)"),
        (
            usecases::EnvOrigin::Sealed,
            "encrypted, not opened (2 values)",
        ),
    ] {
        let view = ProcessView {
            env_origin: origin,
            env_declared: 2,
            ..running_view(0, "web")
        };
        assert_eq!(value_of(&view, "env"), shown);
    }
}

#[test]
fn describe_counts_a_single_value_in_the_singular() {
    let view = ProcessView {
        env_declared: 1,
        ..running_view(0, "web")
    };
    assert_eq!(value_of(&view, "env"), "plain (1 value)");
}

#[test]
fn describe_reports_an_empty_environment_as_none_declared() {
    assert_eq!(value_of(&running_view(0, "web"), "env"), "plain (0 values)");
}

fn shown(key: &str, value: &str, scope: usecases::EnvScope) -> usecases::EnvDisplay {
    usecases::EnvDisplay {
        key: key.to_string(),
        value: usecases::mask_secret(value),
        scope,
    }
}

#[test]
fn describe_lists_every_environment_variable() {
    let view = crate::process_views::view_with_env(
        0,
        "web",
        vec![
            shown(
                "CF_API_TOKEN",
                "abcdefghijklmnopqrstuvwxyz",
                usecases::EnvScope::App,
            ),
            shown("TZ", "UTC", usecases::EnvScope::Global),
        ],
    );
    let rendered = render_describe(&view);
    assert!(rendered.contains("env CF_API_TOKEN"), "got: {rendered}");
    assert!(rendered.contains("env TZ"), "got: {rendered}");
}

#[test]
fn describe_labels_each_variable_with_its_source() {
    let view = crate::process_views::view_with_env(
        0,
        "web",
        vec![
            shown("HOME", "/home/dev", usecases::EnvScope::Injected),
            shown("TZ", "UTC", usecases::EnvScope::Global),
            shown("PORT", "8080", usecases::EnvScope::App),
        ],
    );
    let rendered = render_describe(&view);
    for label in ["(pm3)", "(global)", "(app)"] {
        assert!(rendered.contains(label), "missing {label} in: {rendered}");
    }
}

#[test]
fn describe_shows_only_the_ends_of_a_long_value() {
    let view = crate::process_views::view_with_env(
        0,
        "web",
        vec![shown(
            "CF_API_TOKEN",
            "abcdefghijklmnopqrstuvwxyz",
            usecases::EnvScope::App,
        )],
    );
    let rendered = render_describe(&view);
    assert!(rendered.contains("abcd..wxyz 26"), "got: {rendered}");
    assert!(
        !rendered.contains("abcdefghijkl"),
        "the middle never leaves the daemon, got: {rendered}"
    );
}

#[test]
fn describe_hides_every_character_of_a_short_value() {
    let view = crate::process_views::view_with_env(
        0,
        "web",
        vec![shown("PORT", "8080", usecases::EnvScope::App)],
    );
    let rendered = render_describe(&view);
    assert!(rendered.contains("env PORT  .. 4"), "got: {rendered}");
}

#[test]
fn describe_says_nothing_extra_when_an_app_declares_no_environment() {
    let rendered = render_describe(&running_view(0, "web"));
    assert!(
        !rendered.contains("(app)"),
        "an app without values gets no block, got: {rendered}"
    );
}

#[test]
fn describe_keeps_the_fixed_rows_aligned_when_a_variable_name_is_long() {
    let view = crate::process_views::view_with_env(
        0,
        "web",
        vec![shown(
            "A_VERY_LONG_VARIABLE_NAME_INDEED",
            "8080",
            usecases::EnvScope::App,
        )],
    );
    let rendered = render_describe(&view);
    let gap = " ".repeat("unstable restarts".len() - "id".len() + LABEL_GAP.len());
    assert!(
        rendered.contains(&format!("id{gap}0")),
        "the env block aligns on its own, got: {rendered}"
    );
}

#[test]
fn describe_reports_both_restart_counters() {
    let view = running_view(0, "web");
    assert_eq!(
        value_of(&view, "unstable restarts"),
        crate::process_views::UNSTABLE_RESTARTS.to_string()
    );
    assert_eq!(
        value_of(&view, "restarts"),
        crate::process_views::RESTART_TIME.to_string()
    );
}
