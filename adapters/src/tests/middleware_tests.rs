#![cfg(has_http)]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use super::test_helpers::test_router;
use crate::middleware::{REQUEST_ID_HEADER, classify_status_result};

#[tokio::test]
async fn passthrough_inbound_request_id() {
    let req = Request::builder()
        .uri("/")
        .header(REQUEST_ID_HEADER, "test-id-123")
        .body(Body::empty())
        .expect("build request");

    let res = test_router().oneshot(req).await.expect("send request");
    let returned = res
        .headers()
        .get(REQUEST_ID_HEADER)
        .expect("response carries x-request-id")
        .to_str()
        .expect("ascii");
    assert_eq!(returned, "test-id-123");
}

#[tokio::test]
async fn generate_uuid_v4_when_absent() {
    let req = Request::builder()
        .uri("/")
        .body(Body::empty())
        .expect("build request");

    let res = test_router().oneshot(req).await.expect("send request");
    let returned = res
        .headers()
        .get(REQUEST_ID_HEADER)
        .expect("response carries x-request-id")
        .to_str()
        .expect("ascii");
    let parsed = uuid::Uuid::parse_str(returned).expect("generated id is valid UUID");
    assert_eq!(parsed.get_version_num(), 4);
}

#[tokio::test]
async fn server_error_response_still_carries_request_id() {
    let req = Request::builder()
        .uri("/boom")
        .body(Body::empty())
        .expect("build request");

    let res = test_router().oneshot(req).await.expect("send request");
    assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert!(res.headers().contains_key(REQUEST_ID_HEADER));
}

#[test]
fn classify_2xx_is_ok() {
    assert_eq!(classify_status_result(StatusCode::OK.as_u16()), "ok");
}

#[test]
fn classify_3xx_is_ok() {
    assert_eq!(
        classify_status_result(StatusCode::MOVED_PERMANENTLY.as_u16()),
        "ok"
    );
}

#[test]
fn classify_4xx_is_client_error() {
    assert_eq!(
        classify_status_result(StatusCode::NOT_FOUND.as_u16()),
        "client_error"
    );
}

#[test]
fn classify_5xx_is_server_error() {
    assert_eq!(
        classify_status_result(StatusCode::INTERNAL_SERVER_ERROR.as_u16()),
        "server_error"
    );
}
