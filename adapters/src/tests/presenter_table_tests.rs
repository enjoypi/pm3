use super::*;
use crate::process_views::{RUNNING_PID, idle_view, running_view};

fn rendered_rows(views: &[ProcessView]) -> Vec<String> {
    render_table(views).lines().map(str::to_string).collect()
}

fn header_of(views: &[ProcessView]) -> String {
    rendered_rows(views).first().cloned().expect("header row")
}

fn body_cells(views: &[ProcessView]) -> Vec<String> {
    rendered_rows(views)
        .get(1)
        .expect("body row")
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

#[test]
fn an_empty_table_explains_that_nothing_is_managed() {
    assert_eq!(render_table(&[]), EMPTY_NOTICE);
}

#[test]
fn the_header_names_every_column() {
    let header = header_of(&[running_view(0, "web")]);
    for column in [
        "id", "name", "pid", "status", "↺", "uptime", "rss", "cpu", "next", "sandbox",
    ] {
        assert!(header.contains(column), "{column} missing from: {header}");
    }
}

#[test]
fn a_row_reports_every_field_of_a_running_app() {
    let cells = body_cells(&[running_view(7, "web")]);
    assert_eq!(
        cells,
        vec![
            "7",
            "web",
            &RUNNING_PID.to_string(),
            "online",
            "2",
            "5s",
            "-",
            "-",
            "-",
            "workspace-write",
        ]
    );
}

#[test]
fn a_row_reports_the_sampled_resources() {
    let mut view = running_view(0, "web");
    view.rss_kib = Some(1536);
    view.cpu_tenths = Some(7);
    let cells = body_cells(&[view]);
    assert_eq!(cells.get(6).map(String::as_str), Some("1.5M"));
    assert_eq!(cells.get(7).map(String::as_str), Some("0.7%"));
}

#[test]
fn a_row_reports_every_field_of_an_idle_app() {
    let cells = body_cells(&[idle_view(0, "web")]);
    assert_eq!(
        cells,
        vec![
            "0",
            "web",
            "-",
            "stopped",
            "2",
            "-",
            "-",
            "-",
            "-",
            "workspace-write"
        ]
    );
}

#[test]
fn a_row_marks_an_app_allowed_to_reach_the_network() {
    let mut view = running_view(0, "web");
    view.sandbox_network = true;
    let row = rendered_rows(&[view]).get(1).cloned().expect("body row");
    assert!(row.ends_with("workspace-write+net"), "got: {row}");
}

#[test]
fn every_app_gets_its_own_row() {
    let views = vec![running_view(0, "web"), running_view(1, "db")];
    assert_eq!(rendered_rows(&views).len(), 3);
}

#[test]
fn columns_line_up_across_rows_of_different_widths() {
    let views = vec![running_view(0, "web"), running_view(10, "database")];
    let rows = rendered_rows(&views);
    let name_column: Vec<Option<usize>> = rows
        .iter()
        .map(|row| row.find("web").or_else(|| row.find("database")))
        .collect();
    assert_eq!(name_column, vec![None, Some(4), Some(4)]);
}

#[test]
fn no_row_carries_trailing_padding() {
    let views = vec![running_view(0, "web"), running_view(1, "db")];
    for row in rendered_rows(&views) {
        assert_eq!(row.trim_end(), row, "got padded row: {row:?}");
    }
}

#[test]
fn a_scheduled_row_shows_the_next_clock_time_with_its_offset() {
    let mut view = idle_view(0, "sweep");
    view.next_fire_ms = Some(1_700_000_000_000);
    let cells = body_cells(&[view]);
    let next = cells.get(8).expect("next column present");
    assert!(
        next.contains(':') && (next.contains('+') || next.contains('-')),
        "next column should read as HH:MM±HH:MM, got: {next}"
    );
}
