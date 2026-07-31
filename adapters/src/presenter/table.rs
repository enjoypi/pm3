use usecases::ProcessView;

use super::fields::{format_clock, format_pid, format_sandbox, format_uptime, pad};

pub const EMPTY_NOTICE: &str = "no apps are managed by pm3";

const HEADERS: [&str; 8] = [
    "id", "name", "pid", "status", "↺", "uptime", "next", "sandbox",
];
const COLUMN_GAP: &str = "  ";
const COLUMNS: usize = 8;

type Row = [String; COLUMNS];

#[must_use]
pub fn render_table(views: &[ProcessView]) -> String {
    if views.is_empty() {
        return EMPTY_NOTICE.to_string();
    }
    let mut rows: Vec<Row> = Vec::with_capacity(views.len() + 1);
    rows.push(HEADERS.map(str::to_string));
    rows.extend(views.iter().map(row_of));
    let widths = column_widths(&rows);
    let lines: Vec<String> = rows.iter().map(|row| join_row(row, &widths)).collect();
    lines.join("\n")
}

fn row_of(view: &ProcessView) -> Row {
    [
        view.pm_id.to_string(),
        view.name.clone(),
        format_pid(view.pid),
        view.status.as_str().to_string(),
        view.restart_time.to_string(),
        format_uptime(view.uptime_ms),
        format_clock(view.next_fire_ms),
        format_sandbox(&view.sandbox_mode, view.sandbox_network),
    ]
}

fn column_widths(rows: &[Row]) -> [usize; COLUMNS] {
    let mut widths = [0_usize; COLUMNS];
    for row in rows {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.chars().count());
        }
    }
    widths
}

fn join_row(row: &Row, widths: &[usize; COLUMNS]) -> String {
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
