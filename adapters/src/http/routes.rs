use axum::{
    Router,
    extract::DefaultBodyLimit,
    routing::{get, post},
};

use super::controller::{
    delete, describe, health, list, reset, restart, signal, start, stop, stop_all,
};
use crate::state::DaemonHandle;

pub const REQUEST_ID_HEADER: &str = "x-request-id";
pub const HEALTH_PATH: &str = "/health";
pub const APPS_PATH: &str = "/apps";
pub const SERVICES_STOP_ALL_PATH: &str = "/services/stop-all";

pub const STOP_ACTION: &str = "stop";
pub const RESTART_ACTION: &str = "restart";
pub const RESET_ACTION: &str = "reset";
pub const SIGNAL_ACTION: &str = "signal";

const APP_PATH: &str = "/apps/{selector}";

pub fn router(handle: DaemonHandle, body_limit_bytes: usize) -> Router {
    Router::new()
        .route(HEALTH_PATH, get(health))
        .route(APPS_PATH, get(list).post(start))
        .route(APP_PATH, get(describe).delete(delete))
        .route(&action_route(STOP_ACTION), post(stop))
        .route(&action_route(RESTART_ACTION), post(restart))
        .route(&action_route(RESET_ACTION), post(reset))
        .route(&action_route(SIGNAL_ACTION), post(signal))
        .route(SERVICES_STOP_ALL_PATH, post(stop_all))
        .layer(DefaultBodyLimit::max(body_limit_bytes))
        .with_state(handle)
}

fn action_route(action: &str) -> String {
    format!("{APP_PATH}/{action}")
}
