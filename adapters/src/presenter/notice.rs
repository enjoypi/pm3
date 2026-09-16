use super::table::Listing;
use crate::http::ProcessViewDto;

const NOTICE_GAP: &str = ",";
const SEALED_ENV: &str = "env:sealed";
const BREAKER: &str = "breaker";
const NO_SELF_HEAL: &str = "noselfheal";
const FLAPPING: &str = "flapping";
const ERRORED: &str = "errored";
const SEALED_ORIGIN: &str = "sealed";
const READ_FULL: &str = "read:full";
const FULL_ACCESS_MODE: &str = "danger-full-access";
const READ_ONLY_MODE: &str = "read-only";
const NO_NETWORK: &str = "nonet";
const FULL_ACCESS: &str = "full";
const READ_ONLY: &str = "ro";

#[must_use]
pub fn format_notice(view: &ProcessViewDto, listing: Listing) -> String {
    let mut marks: Vec<String> = Vec::new();
    if let Some(health) = health_mark(view) {
        marks.push(health);
    }
    if view.env_origin == SEALED_ORIGIN {
        marks.push(SEALED_ENV.to_string());
    }
    if listing == Listing::Compact {
        marks.extend(sandbox_marks(view));
    }
    marks.join(NOTICE_GAP)
}

fn health_mark(view: &ProcessViewDto) -> Option<String> {
    if view.status == ERRORED {
        if tripped(view) {
            return Some(format!("{BREAKER}:{}", view.unstable_restarts));
        }
        if !view.autorestart {
            return Some(NO_SELF_HEAL.to_string());
        }
    }
    if view.unstable_restarts > 0 {
        return Some(format!("{FLAPPING}:{}", view.unstable_restarts));
    }
    None
}

const fn tripped(view: &ProcessViewDto) -> bool {
    view.max_restarts > 0 && view.unstable_restarts >= view.max_restarts
}

fn sandbox_marks(view: &ProcessViewDto) -> Vec<String> {
    let mut marks: Vec<String> = Vec::new();
    if view.sandbox_mode == FULL_ACCESS_MODE {
        marks.push(FULL_ACCESS.to_string());
    }
    if view.sandbox_mode == READ_ONLY_MODE {
        marks.push(READ_ONLY.to_string());
    }
    if view.sandbox_read == "full" {
        marks.push(READ_FULL.to_string());
    }
    if !view.sandbox_network {
        marks.push(NO_NETWORK.to_string());
    }
    marks
}

#[must_use]
pub fn local_offset_label() -> String {
    let offset = chrono::Local::now().format("%:z").to_string();
    format!("({offset})")
}

#[cfg(test)]
#[path = "../tests/presenter_notice_tests.rs"]
mod tests;
