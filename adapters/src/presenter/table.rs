use super::{
    fields::{
        format_clock, format_pid, format_resources, format_restarts, format_sandbox_flags,
        format_uptime, pad,
    },
    notice::{format_notice, local_offset_label},
};
use crate::http::ProcessViewDto;

pub const EMPTY_NOTICE: &str = "no apps are managed by pm3";

const COLUMN_GAP: &str = " ";
const COMPACT_COLUMNS: usize = 9;
const FULL_COLUMNS: usize = 10;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Listing {
    Compact,
    Full,
}

impl Listing {
    const fn columns(self) -> usize {
        match self {
            Self::Compact => COMPACT_COLUMNS,
            Self::Full => FULL_COLUMNS,
        }
    }
}

#[must_use]
pub fn render_table(views: &[ProcessViewDto], listing: Listing) -> String {
    if views.is_empty() {
        return EMPTY_NOTICE.to_string();
    }
    let mut rows: Vec<Vec<String>> = Vec::with_capacity(views.len() + 1);
    rows.push(headers(listing));
    let mut ordered: Vec<&ProcessViewDto> = views.iter().collect();
    ordered.sort_by(|left, right| left.name.cmp(&right.name));
    rows.extend(ordered.iter().map(|view| row_of(view, listing)));
    let widths = column_widths(&rows, listing.columns());
    let lines: Vec<String> = rows.iter().map(|row| join_row(row, &widths)).collect();
    lines.join("\n")
}

fn headers(listing: Listing) -> Vec<String> {
    let next = format!("next{}", local_offset_label());
    match listing {
        Listing::Compact => vec![
            "id".to_string(),
            "name".to_string(),
            "status".to_string(),
            "↺".to_string(),
            "uptime".to_string(),
            "rss/cpu".to_string(),
            next,
            "W/R/N".to_string(),
            String::new(),
        ],
        Listing::Full => vec![
            "id".to_string(),
            "name".to_string(),
            "pid".to_string(),
            "status".to_string(),
            "↺".to_string(),
            "uptime".to_string(),
            "rss/cpu".to_string(),
            next,
            "W/R/N".to_string(),
            String::new(),
        ],
    }
}

fn row_of(view: &ProcessViewDto, listing: Listing) -> Vec<String> {
    let mut cells = vec![view.pm_id.to_string(), view.name.clone()];
    if listing == Listing::Full {
        cells.push(format_pid(view.pid));
    }
    cells.push(view.status.clone());
    cells.push(format_restarts(view.unstable_restarts));
    cells.push(format_uptime(view.uptime_ms));
    cells.push(format_resources(view.rss_kib, view.cpu_tenths));
    cells.push(format_clock(view.next_fire_ms));
    cells.push(format_sandbox_flags(
        &view.sandbox_mode,
        &view.sandbox_read,
        view.sandbox_network,
    ));
    cells.push(format_notice(view));
    cells
}

fn column_widths(rows: &[Vec<String>], columns: usize) -> Vec<usize> {
    let mut widths = vec![0_usize; columns];
    for row in rows {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.chars().count());
        }
    }
    widths
}

fn join_row(row: &[String], widths: &[usize]) -> String {
    let cells: Vec<String> = row
        .iter()
        .zip(widths)
        .map(|(cell, width)| pad(cell, *width))
        .collect();
    cells.join(COLUMN_GAP).trim_end().to_string()
}

#[cfg(test)]
#[path = "../tests/presenter_table_tests.rs"]
mod tests;
