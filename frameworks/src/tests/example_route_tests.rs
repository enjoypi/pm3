#![cfg(feature = "sqlite")]

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use tower::ServiceExt as _;

use super::*;
use crate::test_helpers::body_json;

fn init_debug_telemetry() {
    let config = adapters::TelemetryConfig {
        service_name: "example-route-tests".to_string(),
        log_level: "debug".to_string(),
        log_format: adapters::LOG_FORMAT_JSON.to_string(),
    };
    crate::telemetry::init_telemetry(&config).expect("debug telemetry config is valid");
}

async fn migrated_store() -> (adapters::SqlExampleStore, tempfile::TempDir) {
    init_debug_telemetry();
    let dir = tempfile::tempdir().expect("create temp dir");
    let url = crate::test_helpers::sqlite_rwc_url(&dir.path().join("examples.db"));
    let config = adapters::DatabaseConfig {
        url,
        migrations_path: String::new(),
        pool: adapters::PoolConfig {
            max_connections: 5,
            min_connections: 1,
            acquire_timeout_secs: 5,
            idle_timeout_secs: 300,
            max_lifetime_secs: 1800,
            health_check_timeout_secs: 3,
        },
    };

    let pool = crate::database::create_pool(&config)
        .await
        .expect("create pool");
    crate::database::run_migrations(&pool, &crate::test_helpers::workspace_migrations_dir())
        .await
        .expect("run migrations");

    (adapters::SqlExampleStore::new(pool), dir)
}

fn post_example(name: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/examples")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(format!(r#"{{"name":"{name}"}}"#)))
        .expect("request")
}

fn get_example(id: i64) -> Request<Body> {
    Request::builder()
        .uri(format!("/examples/{id}"))
        .body(Body::empty())
        .expect("request")
}

#[tokio::test]
async fn post_examples_returns_201_against_real_sqlite() {
    let (store, _dir) = migrated_store().await;
    let router = build_router_with_examples(AppState::new(), store);

    let response = router
        .oneshot(post_example("widget"))
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn created_example_is_retrievable_by_returned_id() {
    let (store, _dir) = migrated_store().await;
    let router = build_router_with_examples(AppState::new(), store);

    let created = router
        .clone()
        .oneshot(post_example("gadget"))
        .await
        .expect("response");
    let created_body = body_json(created.into_body()).await;
    let id = created_body["id"].as_i64().expect("id in create response");

    let found = router.oneshot(get_example(id)).await.expect("response");
    let found_body = body_json(found.into_body()).await;

    assert_eq!(found_body["name"], "gadget", "got: {found_body}");
}

#[tokio::test]
async fn get_unknown_example_returns_404_against_real_sqlite() {
    let (store, _dir) = migrated_store().await;
    let router = build_router_with_examples(AppState::new(), store);

    let response = router.oneshot(get_example(4_040)).await.expect("response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn post_examples_rejects_blank_name_before_touching_store() {
    let (store, _dir) = migrated_store().await;
    let router = build_router_with_examples(AppState::new(), store);

    let response = router.oneshot(post_example("  ")).await.expect("response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn non_numeric_id_keeps_the_json_error_contract() {
    let (store, _dir) = migrated_store().await;
    let router = build_router_with_examples(AppState::new(), store);
    let request = Request::builder()
        .uri("/examples/not-a-number")
        .body(Body::empty())
        .expect("request");

    let response = router.oneshot(request).await.expect("response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_json(response.into_body()).await;
    assert!(
        body["error"].is_string(),
        "a Path rejection must answer with the ErrorResponse JSON body, got: {body}"
    );
}

#[tokio::test]
async fn malformed_json_body_keeps_the_json_error_contract() {
    let (store, _dir) = migrated_store().await;
    let router = build_router_with_examples(AppState::new(), store);
    let request = Request::builder()
        .method("POST")
        .uri("/examples")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{not json"))
        .expect("request");

    let response = router.oneshot(request).await.expect("response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_json(response.into_body()).await;
    assert!(
        body["error"].is_string(),
        "a Json rejection must answer with the ErrorResponse JSON body, got: {body}"
    );
}

#[tokio::test]
async fn example_routes_absent_when_store_not_wired() {
    let router = build_router(AppState::new());

    let response = router
        .oneshot(post_example("widget"))
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
