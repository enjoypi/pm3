use chrono::{Local, TimeZone as _};

pub const MISSING: &str = "-";
pub const NETWORK_SUFFIX: &str = "+net";

const LIST_SEPARATOR: &str = ", ";
const CLOCK_FORMAT: &str = "%H:%M%:z";
const STAMP_FORMAT: &str = "%Y-%m-%d %H:%M:%S UTC%:z";
const MS_PER_SECOND: u64 = 1000;
const SECONDS_PER_MINUTE: u64 = 60;
const MINUTES_PER_HOUR: u64 = 60;
const HOURS_PER_DAY: u64 = 24;
const KIB_PER_MIB: u64 = 1024;
const TENTHS_PER_PERCENT: u32 = 10;

pub fn format_memory(rss_kib: Option<u64>) -> String {
    let Some(kib) = rss_kib else {
        return MISSING.to_string();
    };
    if kib >= KIB_PER_MIB {
        let tenths = kib.saturating_mul(TENTHS_PER_PERCENT.into()) / KIB_PER_MIB;
        return format!(
            "{}.{}M",
            tenths / u64::from(TENTHS_PER_PERCENT),
            tenths % u64::from(TENTHS_PER_PERCENT)
        );
    }
    format!("{kib}K")
}

pub fn format_cpu(tenths: Option<u32>) -> String {
    let Some(tenths) = tenths else {
        return MISSING.to_string();
    };
    format!(
        "{}.{}%",
        tenths / TENTHS_PER_PERCENT,
        tenths % TENTHS_PER_PERCENT
    )
}

pub fn format_pid(pid: Option<u32>) -> String {
    pid.map_or_else(|| MISSING.to_string(), |value| value.to_string())
}

pub fn format_uptime(uptime_ms: Option<u64>) -> String {
    let Some(elapsed_ms) = uptime_ms else {
        return MISSING.to_string();
    };
    let seconds = elapsed_ms / MS_PER_SECOND;
    if seconds == 0 {
        return format!("{elapsed_ms}ms");
    }
    if seconds < SECONDS_PER_MINUTE {
        return format!("{seconds}s");
    }
    let minutes = seconds / SECONDS_PER_MINUTE;
    if minutes < MINUTES_PER_HOUR {
        return format!("{minutes}m");
    }
    let hours = minutes / MINUTES_PER_HOUR;
    if hours < HOURS_PER_DAY {
        return format!("{hours}h");
    }
    format!("{}d", hours / HOURS_PER_DAY)
}

pub fn format_clock(at_ms: Option<u64>) -> String {
    format_instant(at_ms, CLOCK_FORMAT)
}

pub fn format_stamp(at_ms: Option<u64>) -> String {
    format_instant(at_ms, STAMP_FORMAT)
}

fn format_instant(at_ms: Option<u64>, layout: &str) -> String {
    let Some(at_ms) = at_ms else {
        return MISSING.to_string();
    };
    let Ok(millis) = i64::try_from(at_ms) else {
        return MISSING.to_string();
    };
    let Some(moment) = Local.timestamp_millis_opt(millis).single() else {
        return MISSING.to_string();
    };
    moment.format(layout).to_string()
}

pub fn format_sandbox(mode: &str, network: bool) -> String {
    if network {
        return format!("{mode}{NETWORK_SUFFIX}");
    }
    mode.to_string()
}

pub fn format_list(items: &[String]) -> String {
    if items.is_empty() {
        return MISSING.to_string();
    }
    items.join(LIST_SEPARATOR)
}

pub fn pad(cell: &str, width: usize) -> String {
    format!("{cell:<width$}")
}

pub fn widest(cells: impl Iterator<Item = usize>) -> usize {
    cells.max().unwrap_or(0)
}

#[cfg(test)]
#[path = "../tests/presenter_fields_tests.rs"]
mod tests;
