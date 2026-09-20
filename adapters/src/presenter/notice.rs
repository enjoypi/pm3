use crate::http::ProcessViewDto;

const NOTICE_GAP: &str = ",";
const SEALED_ENV: &str = "env:sealed";
const BREAKER: &str = "breaker";
const NO_SELF_HEAL: &str = "noselfheal";
const FLAPPING: &str = "flapping";
const ERRORED: &str = "errored";
const SEALED_ORIGIN: &str = "sealed";
#[must_use]
pub fn format_notice(view: &ProcessViewDto) -> String {
    let mut marks: Vec<String> = Vec::new();
    if let Some(health) = health_mark(view) {
        marks.push(health);
    }
    if view.env_origin == SEALED_ORIGIN {
        marks.push(SEALED_ENV.to_string());
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
    if view.max_restarts == 0 {
        return false;
    }
    view.unstable_restarts >= view.max_restarts
}

#[must_use]
pub fn local_offset_label() -> String {
    let offset = chrono::Local::now().format("%:z").to_string();
    format!("({offset})")
}

#[cfg(test)]
#[path = "../tests/presenter_notice_tests.rs"]
mod tests;
