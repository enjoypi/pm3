use axum::{
    Router,
    routing::{get, post},
};

use super::controller::{delete, describe, health, list, restart, start, stop};
use crate::state::DaemonHandle;

pub const HEALTH_PATH: &str = "/health";
pub const APPS_PATH: &str = "/apps";

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
        .with_state(handle)
}
