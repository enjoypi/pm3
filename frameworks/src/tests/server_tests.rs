#![cfg(unix)]
use axum::{Router, routing::get};
use tokio::{
    io::AsyncWriteExt as _,
    net::{TcpListener, TcpStream, UnixListener},
};

use super::*;

const PROBE_PATH: &str = "/probe";
const PARTIAL_REQUEST: &[u8] = b"GET /probe HTTP/1.1\r\nHost: localhost\r\n";

#[allow(clippy::unused_async, reason = "axum Handler trait requires a future")]
async fn probe() -> &'static str {
    "ok"
}

fn probe_router() -> Router {
    Router::new().route(PROBE_PATH, get(probe))
}

async fn tcp_listener() -> TcpListener {
    TcpListener::bind("127.0.0.1:0").await.expect("bind")
}

#[test]
fn a_serve_failure_explains_itself() {
    let err = ServerError::Serve(std::io::Error::other("boom"));
    assert!(
        err.to_string().contains("cannot serve requests"),
        "got: {err}"
    );
}

#[tokio::test]
async fn serving_a_tcp_listener_stops_on_the_shutdown_signal() {
    let listener = tcp_listener().await;
    serve_listener(listener, probe_router(), async {}, Duration::from_secs(1))
        .await
        .expect("graceful shutdown");
}

#[tokio::test]
async fn serving_a_unix_listener_stops_on_the_shutdown_signal() {
    let dir = tempfile::tempdir().expect("temp dir");
    let listener = UnixListener::bind(dir.path().join("pm3.sock")).expect("bind");
    serve_listener(listener, probe_router(), async {}, Duration::from_secs(1))
        .await
        .expect("graceful shutdown");
}

#[tokio::test]
async fn a_unix_listener_answers_requests() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = dir.path().join("pm3.sock");
    let listener = UnixListener::bind(&socket).expect("bind");
    let (shutdown, wait) = tokio::sync::oneshot::channel::<()>();
    let served = tokio::spawn(async move {
        serve_listener(
            listener,
            probe_router(),
            async move {
                wait.await.ok();
            },
            Duration::from_secs(5),
        )
        .await
    });

    let mut stream = tokio::net::UnixStream::connect(&socket)
        .await
        .expect("connect");
    stream
        .write_all(b"GET /probe HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .expect("write");
    let mut raw = String::new();
    tokio::io::AsyncReadExt::read_to_string(&mut stream, &mut raw)
        .await
        .expect("read");
    assert!(raw.contains("200 OK"), "got: {raw}");

    shutdown.send(()).expect("signal shutdown");
    served.await.expect("join").expect("serve ok");
}

#[tokio::test]
async fn a_connection_that_finishes_inside_the_window_shuts_down_cleanly() {
    let listener = tcp_listener().await;
    let port = listener.local_addr().expect("addr").port();
    let (shutdown, wait) = tokio::sync::oneshot::channel::<()>();
    let served = tokio::spawn(async move {
        serve_listener(
            listener,
            probe_router(),
            async move {
                wait.await.ok();
            },
            Duration::from_secs(10),
        )
        .await
    });

    let mut stream = TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .expect("connect");
    stream.write_all(PARTIAL_REQUEST).await.expect("write");
    tokio::time::sleep(Duration::from_millis(50)).await;
    shutdown.send(()).expect("signal shutdown");
    tokio::time::sleep(Duration::from_millis(100)).await;
    drop(stream);

    let outcome = tokio::time::timeout(Duration::from_secs(5), served)
        .await
        .expect("server exits inside the drain window")
        .expect("join");
    assert!(outcome.is_ok(), "got: {outcome:?}");
}

#[tokio::test]
async fn a_connection_that_outlives_the_window_is_dropped() {
    let listener = tcp_listener().await;
    let port = listener.local_addr().expect("addr").port();
    let (shutdown, wait) = tokio::sync::oneshot::channel::<()>();
    let served = tokio::spawn(async move {
        serve_listener(
            listener,
            probe_router(),
            async move {
                wait.await.ok();
            },
            Duration::from_millis(50),
        )
        .await
    });

    let mut stream = TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .expect("connect");
    stream.write_all(PARTIAL_REQUEST).await.expect("write");
    tokio::time::sleep(Duration::from_millis(50)).await;
    shutdown.send(()).expect("signal shutdown");

    let outcome = tokio::time::timeout(Duration::from_secs(5), served)
        .await
        .expect("server exits after the drain timeout")
        .expect("join");
    assert!(outcome.is_ok(), "got: {outcome:?}");
    drop(stream);
}
