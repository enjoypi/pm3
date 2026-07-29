#![cfg(has_database)]

use std::time::Duration;

use super::*;
use crate::{DbConnection, database};

#[test]
fn readiness_status_code_is_unavailable_for_failed() {
    let readiness = DbReadiness::Failed(serde_json::json!({"status": "error: down"}));
    assert_eq!(
        readiness_status_code(&readiness),
        StatusCode::SERVICE_UNAVAILABLE
    );
}

#[test]
fn readiness_status_code_is_ok_for_connected() {
    let readiness = DbReadiness::Connected(serde_json::json!({"status": "connected"}));
    assert_eq!(readiness_status_code(&readiness), StatusCode::OK);
}

#[test]
fn readiness_status_code_is_ok_for_inactive() {
    let readiness = DbReadiness::Inactive(DB_LABEL_NOT_CONFIGURED);
    assert_eq!(readiness_status_code(&readiness), StatusCode::OK);
}

#[test]
fn result_for_connected_is_ok() {
    let readiness = DbReadiness::Connected(serde_json::json!({}));
    assert_eq!(readiness.result(), READINESS_RESULT_OK);
}

#[test]
fn result_for_failed_is_error() {
    let readiness = DbReadiness::Failed(serde_json::json!({}));
    assert_eq!(readiness.result(), READINESS_RESULT_ERROR);
}

#[test]
fn result_for_inactive_is_disabled() {
    let readiness = DbReadiness::Inactive(DB_LABEL_NOT_CONFIGURED);
    assert_eq!(readiness.result(), READINESS_RESULT_DISABLED);
}

#[test]
fn into_body_keeps_health_payload_for_connected() {
    let readiness = DbReadiness::Connected(serde_json::json!({"status": "connected"}));
    assert_eq!(
        readiness.into_body(),
        serde_json::json!({"status": "connected"})
    );
}

#[test]
fn into_body_keeps_health_payload_for_failed() {
    let readiness = DbReadiness::Failed(serde_json::json!({"status": "error: down"}));
    assert_eq!(
        readiness.into_body(),
        serde_json::json!({"status": "error: down"})
    );
}

#[test]
fn into_body_renders_label_for_inactive() {
    let readiness = DbReadiness::Inactive(DB_LABEL_NOT_CONFIGURED);
    assert_eq!(readiness.into_body(), serde_json::json!("not configured"));
}

#[tokio::test]
async fn readiness_returns_service_unavailable_when_db_unhealthy() {
    let (pool, _dir) = database::test_pool_in_tempdir().await;
    pool.close().await;
    let state = AppState {
        db: Some(DbConnection {
            pool,
            expected_migrations: 0,
            health_check_timeout: Duration::from_secs(1),
        }),
    };
    let (status, _body) = readiness(State(state)).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn readiness_returns_service_unavailable_when_migrations_pending() {
    let (pool, _dir) = database::test_pool_in_tempdir().await;
    let state = AppState {
        db: Some(DbConnection {
            pool: pool.clone(),
            expected_migrations: 1,
            health_check_timeout: Duration::from_secs(3),
        }),
    };

    let (status, _body) = readiness(State(state)).await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    pool.close().await;
}

#[tokio::test]
async fn readiness_returns_ok_when_db_absent() {
    let (status, _body) = readiness(State(AppState::new())).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn db_readiness_with_pool_reports_connected() {
    let (pool, _dir) = database::test_pool_in_tempdir().await;
    let state = AppState {
        db: Some(DbConnection {
            pool: pool.clone(),
            expected_migrations: 0,
            health_check_timeout: Duration::from_secs(3),
        }),
    };
    let readiness = db_readiness(&state).await;
    assert_eq!(readiness.result(), READINESS_RESULT_OK);
    assert_eq!(readiness.into_body()["status"], "connected");
    pool.close().await;
}

#[tokio::test]
async fn db_readiness_without_pool_reports_not_configured() {
    let readiness = db_readiness(&AppState::new()).await;
    assert_eq!(readiness.into_body(), serde_json::json!("not configured"));
}
