use super::*;
use crate::{
    http::ProcessViewDto,
    process_views::{
        MAX_RESTARTS, RESTART_TIME, RUNNING_PID, UNSTABLE_RESTARTS, idle_view, running_view,
    },
};

fn dto(view: &usecases::ProcessView) -> ProcessViewDto {
    ProcessViewDto::from(view)
}

fn compact(views: &[ProcessViewDto]) -> Vec<String> {
    render_table(views, Listing::Compact)
        .lines()
        .map(str::to_string)
        .collect()
}

fn full(views: &[ProcessViewDto]) -> Vec<String> {
    render_table(views, Listing::Full)
        .lines()
        .map(str::to_string)
        .collect()
}

fn body_cells(views: &[ProcessViewDto]) -> Vec<String> {
    compact(views)
        .get(1)
        .expect("body row")
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

fn compact_box(view: ProcessViewDto) -> String {
    body_cells(&[view]).get(7).cloned().expect("box column")
}

fn online(pm_id: u32, name: &str) -> ProcessViewDto {
    dto(&running_view(pm_id, name))
}

#[test]
fn an_empty_table_explains_that_nothing_is_managed() {
    assert_eq!(render_table(&[], Listing::Compact), EMPTY_NOTICE);
}

#[test]
fn the_compact_header_leaves_the_pid_out_and_keeps_the_sandbox() {
    let header = compact(&[online(0, "web")])
        .first()
        .cloned()
        .expect("header row");
    for column in [
        "id", "name", "status", "↺", "uptime", "rss/cpu", "next", "W/R/N",
    ] {
        assert!(header.contains(column), "missing {column} in: {header}");
    }
    assert!(!header.contains("pid"), "got: {header}");
}

#[test]
fn the_full_header_adds_the_pid_and_the_sandbox() {
    let header = full(&[online(0, "web")])
        .first()
        .cloned()
        .expect("header row");
    assert!(header.contains("pid"), "got: {header}");
    assert!(header.contains("W/R/N"), "got: {header}");
}

#[test]
fn the_next_column_names_the_local_offset() {
    let header = compact(&[online(0, "web")])
        .first()
        .cloned()
        .expect("header row");
    assert!(
        header.contains("next(+") || header.contains("next(-"),
        "the header carries the offset so the times need none, got: {header}"
    );
}

#[test]
fn the_restart_column_shows_the_recent_instability_not_the_lifetime_total() {
    let cells = body_cells(&[online(7, "web")]);
    assert_ne!(
        RESTART_TIME, UNSTABLE_RESTARTS,
        "the fixture must tell the two counters apart"
    );
    assert_eq!(cells[3], UNSTABLE_RESTARTS.to_string(), "got: {cells:?}");
}

#[test]
fn a_settled_app_leaves_the_restart_column_empty() {
    let mut view = online(7, "web");
    view.unstable_restarts = 0;
    let cells = body_cells(&[view]);
    assert_eq!(cells[3], "-", "got: {cells:?}");
}

#[test]
fn the_resources_share_one_column() {
    let mut view = online(7, "web");
    view.rss_kib = Some(1536);
    view.cpu_tenths = Some(7);
    let cells = body_cells(&[view]);
    assert_eq!(cells[5], "1.5M/0.7%", "got: {cells:?}");
}

#[test]
fn an_app_without_a_sample_leaves_the_resources_empty() {
    let cells = body_cells(&[dto(&idle_view(7, "web"))]);
    assert_eq!(cells[5], "-", "got: {cells:?}");
}

#[test]
fn the_full_listing_shows_the_pid() {
    let rows = full(&[online(7, "web")]);
    let cells: Vec<&str> = rows[1].split_whitespace().collect();
    assert_eq!(cells[2], RUNNING_PID.to_string(), "got: {cells:?}");
}

#[test]
fn the_rows_come_out_in_name_order() {
    let rows = compact(&[online(2, "web"), online(1, "api")]);
    let names: Vec<&str> = rows
        .iter()
        .skip(1)
        .filter_map(|row| row.split_whitespace().nth(1))
        .collect();
    assert_eq!(names, ["api", "web"], "got: {names:?}");
}

#[test]
fn a_healthy_app_says_nothing_in_the_notice_column() {
    let mut view = online(7, "web");
    view.unstable_restarts = 0;
    view.sandbox_network = true;
    let row = compact(&[view]).remove(1);
    assert!(!row.contains("flapping"), "got: {row}");
    assert_eq!(row.trim_end(), row, "no row carries trailing padding");
}

#[test]
fn a_tripped_breaker_names_itself_in_the_notice_column() {
    let mut view = online(7, "web");
    view.status = "errored".to_string();
    view.unstable_restarts = MAX_RESTARTS;
    let row = compact(&[view]).remove(1);
    assert!(
        row.contains(&format!("breaker:{MAX_RESTARTS}")),
        "got: {row}"
    );
}

#[test]
fn an_app_that_declined_autorestart_says_so_when_it_errors() {
    let mut view = online(7, "web");
    view.status = "errored".to_string();
    view.autorestart = false;
    view.unstable_restarts = 0;
    let row = compact(&[view]).remove(1);
    assert!(row.contains("noselfheal"), "got: {row}");
}

#[test]
fn a_flapping_app_names_its_recent_restarts() {
    let row = compact(&[online(7, "web")]).remove(1);
    assert!(
        row.contains(&format!("flapping:{UNSTABLE_RESTARTS}")),
        "got: {row}"
    );
}

#[test]
fn the_compact_listing_reports_full_access_in_the_box() {
    let mut view = online(7, "web");
    view.sandbox_mode = "danger-full-access".to_string();
    view.sandbox_network = true;
    assert_eq!(compact_box(view), "F/-/N");
}

#[test]
fn the_compact_listing_stays_quiet_about_a_default_sandbox() {
    let mut view = online(7, "web");
    view.sandbox_network = true;
    view.unstable_restarts = 0;
    let row = compact(std::slice::from_ref(&view)).remove(1);
    assert_eq!(compact_box(view), "W/-/N");
    assert!(!row.contains("nonet"), "got: {row}");
    assert!(!row.contains("read:full"), "got: {row}");
}

#[test]
fn a_confined_app_without_network_uses_an_empty_network_flag() {
    let mut view = online(7, "web");
    view.sandbox_network = false;
    let row = compact(std::slice::from_ref(&view)).remove(1);
    assert_eq!(compact_box(view), "W/-/-");
    assert!(!row.contains("nonet"), "got: {row}");
}

#[test]
fn a_full_read_scope_sets_the_read_flag() {
    let mut view = online(7, "web");
    view.sandbox_read = "full".to_string();
    view.sandbox_network = true;
    let row = compact(std::slice::from_ref(&view)).remove(1);
    assert_eq!(compact_box(view), "W/R/N");
    assert!(!row.contains("read:full"), "got: {row}");
}

#[test]
fn a_sealed_environment_still_shows_in_the_notice_column() {
    let mut view = online(7, "web");
    view.env_origin = "sealed".to_string();
    view.sandbox_network = true;
    let row = compact(&[view]).remove(1);
    assert!(row.contains("env:sealed"), "got: {row}");
}

#[test]
fn the_full_listing_keeps_the_sandbox_out_of_the_notice_column() {
    let mut view = online(7, "web");
    view.sandbox_network = false;
    let row = full(&[view]).remove(1);
    assert!(
        !row.contains("nonet"),
        "the box column already says it, got: {row}"
    );
}

#[test]
fn every_app_gets_one_row() {
    let rows = compact(&[online(0, "web"), online(1, "api")]);
    assert_eq!(rows.len(), 3, "got: {rows:?}");
}

#[test]
fn the_columns_line_up_across_rows() {
    let rows = compact(&[online(0, "web"), online(1, "gateway")]);
    let starts: Vec<Option<usize>> = rows.iter().map(|row| row.find("status")).collect();
    assert_eq!(starts[0], Some(rows[0].find("status").expect("header")));
    let name_column: Vec<Option<usize>> = rows.iter().map(|row| row.find(' ')).collect();
    assert_eq!(
        name_column[1], name_column[2],
        "the name column starts alike on every row: {rows:?}"
    );
}

#[test]
fn the_full_listing_renders_the_sandbox_flags() {
    let mut view = online(7, "web");
    view.sandbox_network = true;
    let rows = full(&[view]);
    let cells: Vec<&str> = rows[1].split_whitespace().collect();
    assert_eq!(cells[8], "W/-/N", "got: {cells:?}");
}

#[test]
fn a_read_only_app_uses_an_empty_write_flag() {
    let mut view = online(7, "web");
    view.sandbox_mode = "read-only".to_string();
    view.sandbox_network = true;
    let row = compact(std::slice::from_ref(&view)).remove(1);
    assert_eq!(compact_box(view), "-/-/N");
    assert!(
        !row.split_whitespace().any(|cell| cell == "ro"),
        "got: {row}"
    );
}

#[test]
fn an_unlimited_breaker_reads_as_flapping_not_tripped() {
    let mut view = online(7, "web");
    view.status = "errored".to_string();
    view.max_restarts = 0;
    view.unstable_restarts = 900;
    let row = compact(&[view]).remove(1);
    assert!(
        row.contains("flapping:900"),
        "a service that never gives up is flapping, not tripped: {row}"
    );
    assert!(!row.contains("breaker"), "got: {row}");
}

#[test]
fn an_errored_app_that_still_self_heals_reads_as_flapping() {
    let mut view = online(7, "web");
    view.status = "errored".to_string();
    view.unstable_restarts = 1;
    let row = compact(&[view]).remove(1);
    assert!(row.contains("flapping:1"), "got: {row}");
    assert!(!row.contains("noselfheal"), "got: {row}");
}

#[test]
fn an_errored_app_below_the_breaker_limit_reads_as_flapping() {
    let mut view = online(7, "web");
    view.status = "errored".to_string();
    view.unstable_restarts = MAX_RESTARTS - 1;
    let row = compact(&[view]).remove(1);
    assert!(!row.contains("breaker"), "got: {row}");
}
