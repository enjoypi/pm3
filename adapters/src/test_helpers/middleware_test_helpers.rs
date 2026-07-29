use axum::{Router, http::StatusCode, routing::get};

use crate::middleware;

async fn ok_handler() -> &'static str {
    "ok"
}

async fn boom_handler() -> StatusCode {
    StatusCode::INTERNAL_SERVER_ERROR
}

pub fn test_router() -> Router {
    Router::new()
        .route("/", get(ok_handler))
        .route("/boom", get(boom_handler))
        .layer(axum::middleware::from_fn(middleware::request_id))
}
