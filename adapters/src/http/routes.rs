use axum::{
    Router,
    routing::{get, post},
};

use super::controller::{delete, describe, health, list, restart, start, stop, stop_all};
use crate::state::DaemonHandle;

pub const REQUEST_ID_HEADER: &str = "x-request-id";
pub const HEALTH_PATH: &str = "/health";
pub const APPS_PATH: &str = "/apps";
pub const SERVICES_STOP_ALL_PATH: &str = "/services/stop-all";

const APP_PATH: &str = "/apps/{selector}";
const APP_STOP_PATH: &str = "/apps/{selector}/stop";
const APP_RESTART_PATH: &str = "/apps/{selector}/restart";

pub fn router(handle: DaemonHandle) -> Router {
    Router::new()
        .route(HEALTH_PATH, get(health))
        .route(APPS_PATH, get(list).post(start))
        .route(APP_PATH, get(describe).delete(delete))
        .route(APP_STOP_PATH, post(stop))
        .route(APP_RESTART_PATH, post(restart))
        .route(SERVICES_STOP_ALL_PATH, post(stop_all))
        .with_state(handle)
}
