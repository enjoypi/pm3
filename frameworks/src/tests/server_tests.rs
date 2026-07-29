use std::time::Duration;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use super::*;
use crate::test_helpers::body_json;

#[tokio::test]
async fn health_endpoint_returns_200() {
    let router = build_router(AppState::new());
    let response = router
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn readiness_endpoint_returns_200() {
    let router = build_router(AppState::new());
    let response = router
        .oneshot(
            Request::builder()
                .uri("/readiness")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);

    let json = body_json(response.into_body()).await;
    assert_eq!(json["config"], "loaded");
}

#[tokio::test]
async fn request_id_generated_when_missing() {
    let router = build_router(AppState::new());
    let response = router
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    let request_id = response.headers().get("x-request-id");
    assert!(request_id.is_some(), "should have x-request-id header");
    let id_str = request_id.expect("header").to_str().expect("utf8");
    assert!(!id_str.is_empty());
}

#[tokio::test]
async fn request_id_passthrough_when_present() {
    let router = build_router(AppState::new());
    let response = router
        .oneshot(
            Request::builder()
                .uri("/health")
                .header("x-request-id", "my-custom-id")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    let request_id = response
        .headers()
        .get("x-request-id")
        .expect("header")
        .to_str()
        .expect("utf8");
    assert_eq!(request_id, "my-custom-id");
}

#[tokio::test]
async fn unknown_route_returns_404() {
    let router = build_router(AppState::new());
    let response = router
        .oneshot(
            Request::builder()
                .uri("/nonexistent")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn start_server_returns_bind_error_on_invalid_host() {
    let cfg = adapters::ServerConfig {
        host: "this-is-not.a.valid.host.ever-".to_string(),
        port: 1,
        drain_timeout_secs: 1,
    };
    let router = build_router(AppState::new());
    let result = start_server(&cfg, router, async {}, Duration::from_secs(1)).await;
    let err = result.unwrap_err();
    assert!(matches!(err, ServerError::Bind { .. }), "got: {err}");
}

#[tokio::test]
async fn serve_listener_graceful_shutdown_returns_ok() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let router = build_router(AppState::new());
    serve_listener(listener, router, async {}, Duration::from_secs(1))
        .await
        .expect("graceful shutdown ok");
}

#[tokio::test]
async fn serve_listener_drain_completes_within_window_returns_ok() {
    use tokio::io::AsyncWriteExt as _;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let router = build_router(AppState::new());
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    let server_handle = tokio::spawn(async move {
        serve_listener(
            listener,
            router,
            async move {
                let _ = shutdown_rx.await;
            },
            Duration::from_secs(10),
        )
        .await
    });

    let mut sock = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .expect("connect");
    sock.write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\n")
        .await
        .expect("partial write");
    tokio::time::sleep(Duration::from_millis(50)).await;

    shutdown_tx.send(()).expect("send shutdown");
    tokio::time::sleep(Duration::from_millis(200)).await;
    drop(sock);

    let result = tokio::time::timeout(Duration::from_secs(5), server_handle)
        .await
        .expect("server should exit within drain window")
        .expect("join ok");
    assert!(
        result.is_ok(),
        "drain-completes path returns Ok: {result:?}"
    );
}

#[tokio::test]
async fn serve_listener_drain_timeout_exceeded_walks_warn_branch() {
    use tokio::io::AsyncWriteExt as _;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let router = build_router(AppState::new());
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    let server_handle = tokio::spawn(async move {
        serve_listener(
            listener,
            router,
            async move {
                let _ = shutdown_rx.await;
            },
            Duration::from_millis(50),
        )
        .await
    });

    let mut sock = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .expect("connect");
    sock.write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\n")
        .await
        .expect("partial write");
    tokio::time::sleep(Duration::from_millis(50)).await;

    shutdown_tx.send(()).expect("send shutdown");

    let result = tokio::time::timeout(Duration::from_secs(5), server_handle)
        .await
        .expect("server should exit after drain timeout")
        .expect("join ok");
    assert!(result.is_ok(), "drain timeout path returns Ok: {result:?}");
    drop(sock);
}
