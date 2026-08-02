use tokio::net::{UnixListener, UnixStream};

use super::*;

const REQUEST_ID: &str = "7-1";

const REPLY_200: &[u8] =
    b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 2\r\n\r\nok";
const REQUEST_SINK: usize = 1024;
const TIMEOUT_MS: u64 = 30_000;
const STALL_TIMEOUT_MS: u64 = 20;

fn socket_in(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().join("pm3.sock")
}

fn bound_socket(socket: &Path) -> UnixListener {
    UnixListener::bind(socket).expect("bind")
}

async fn serve_once(listener: UnixListener, reply: &'static [u8]) {
    let (mut stream, _addr) = listener.accept().await.expect("accept");
    let mut sink = vec![0_u8; REQUEST_SINK];
    let read = stream.read(&mut sink).await.unwrap_or_default();
    sink.truncate(read);
    stream.write_all(reply).await.ok();
    stream.shutdown().await.ok();
}

#[test]
fn a_request_without_a_body_declares_no_content_length() {
    let request = http_request("GET", "/apps", None, REQUEST_ID);
    assert_eq!(
        request,
        "GET /apps HTTP/1.1\r\nHost: localhost\r\nx-request-id: 7-1\r\nConnection: close\r\n\r\n"
    );
}

#[test]
fn every_request_carries_a_distinct_correlation_id() {
    let first = next_request_id();
    let second = next_request_id();
    assert_ne!(first, second);
}

#[test]
fn a_request_with_a_body_declares_its_length() {
    let request = http_request("POST", "/apps", Some("{}"), REQUEST_ID);
    assert!(
        request.ends_with("Content-Length: 2\r\n\r\n{}"),
        "{request}"
    );
}

#[test]
fn a_well_formed_reply_yields_its_status_and_body() {
    let reply = parse_http_response("HTTP/1.1 201 Created\r\n\r\nbody").expect("should parse");
    assert_eq!(
        reply,
        HttpReply {
            status: 201,
            body: "body".to_string(),
        }
    );
}

#[test]
fn a_reply_without_a_blank_line_is_rejected() {
    let err = parse_http_response("HTTP/1.1 200 OK\r\n")
        .unwrap_err()
        .to_string();
    assert!(err.contains("not terminated by a blank line"), "got: {err}");
}

#[test]
fn a_reply_without_a_status_line_is_rejected() {
    let err = parse_http_response("\r\n\r\nbody").unwrap_err().to_string();
    assert!(err.contains("the status line is missing"), "got: {err}");
}

#[test]
fn a_status_line_without_a_code_is_rejected() {
    let err = parse_http_response("HTTP/1.1\r\n\r\n")
        .unwrap_err()
        .to_string();
    assert!(err.contains("carries no status code"), "got: {err}");
}

#[test]
fn a_non_numeric_status_code_is_rejected() {
    let err = parse_http_response("HTTP/1.1 abc OK\r\n\r\n")
        .unwrap_err()
        .to_string();
    assert!(err.contains("'abc' is not a number"), "got: {err}");
}

#[tokio::test]
async fn a_request_reaches_the_daemon_and_returns_its_reply() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = socket_in(&dir);
    let listener = bound_socket(&socket);
    let served = tokio::spawn(serve_once(listener, REPLY_200));

    let reply = UdsClient::new(socket, TIMEOUT_MS)
        .request("GET", "/apps", None)
        .await
        .expect("should talk");
    served.await.expect("join");
    assert_eq!(
        reply,
        HttpReply {
            status: 200,
            body: "ok".to_string(),
        }
    );
}

#[tokio::test]
async fn a_missing_socket_is_reported() {
    let dir = tempfile::tempdir().expect("temp dir");
    let err = UdsClient::new(socket_in(&dir), TIMEOUT_MS)
        .request("GET", "/apps", None)
        .await
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("cannot connect to the pm3 daemon"),
        "got: {err}"
    );
}

#[tokio::test]
async fn a_daemon_that_closes_before_reading_the_request_is_reported() {
    let (mut client_side, server_side) = UnixStream::pair().expect("pair");
    drop(server_side);
    let err = converse(
        &mut client_side,
        "GET /apps HTTP/1.1\r\n\r\n",
        "/tmp/pm3.sock",
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(
        err.contains("cannot send the request to the pm3 daemon"),
        "got: {err}"
    );
}

#[tokio::test]
async fn a_daemon_reply_that_is_not_text_is_reported() {
    let (mut client_side, mut server_side) = UnixStream::pair().expect("pair");
    let server = tokio::spawn(async move {
        let mut sink = vec![0_u8; 64];
        let read = server_side.read(&mut sink).await.unwrap_or_default();
        sink.truncate(read);
        server_side
            .write_all(&[0xff, 0xfe])
            .await
            .expect("write junk");
    });
    let err = converse(
        &mut client_side,
        "GET /apps HTTP/1.1\r\n\r\n",
        "/tmp/pm3.sock",
    )
    .await
    .unwrap_err()
    .to_string();
    server.await.expect("join");
    assert!(err.contains("valid UTF-8"), "got: {err}");
}

#[tokio::test]
async fn a_daemon_that_answers_nothing_is_reported() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = socket_in(&dir);
    let listener = bound_socket(&socket);
    let served = tokio::spawn(serve_once(listener, b""));

    let err = UdsClient::new(socket, TIMEOUT_MS)
        .request("GET", "/apps", None)
        .await
        .unwrap_err()
        .to_string();
    served.await.expect("join");
    assert!(err.contains("answered nothing"), "got: {err}");
}

#[tokio::test]
async fn a_healthy_daemon_is_recognised() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = socket_in(&dir);
    let listener = bound_socket(&socket);
    let served = tokio::spawn(serve_once(listener, REPLY_200));

    let healthy = UdsClient::new(socket, TIMEOUT_MS).daemon_is_healthy().await;
    served.await.expect("join");
    assert!(healthy);
}

#[tokio::test]
async fn an_absent_daemon_is_not_healthy() {
    let dir = tempfile::tempdir().expect("temp dir");
    assert!(
        !UdsClient::new(socket_in(&dir), TIMEOUT_MS)
            .daemon_is_healthy()
            .await
    );
}

#[tokio::test]
async fn a_daemon_that_never_answers_is_given_up_on() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = socket_in(&dir);
    let listener = bound_socket(&socket);
    let stalled = tokio::spawn(async move {
        let (stream, _addr) = listener.accept().await.expect("accept");
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        drop(stream);
    });

    let err = UdsClient::new(socket, STALL_TIMEOUT_MS)
        .request("GET", "/apps", None)
        .await
        .unwrap_err()
        .to_string();
    stalled.abort();
    assert!(
        err.contains(&format!("within {STALL_TIMEOUT_MS} ms")),
        "got: {err}"
    );
}

#[tokio::test]
async fn a_daemon_that_answers_garbage_is_reported() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket = socket_in(&dir);
    let listener = bound_socket(&socket);
    let served = tokio::spawn(serve_once(listener, b"not http at all"));

    let err = UdsClient::new(socket, TIMEOUT_MS)
        .request("GET", "/apps", None)
        .await
        .unwrap_err()
        .to_string();
    served.await.expect("join");
    assert!(
        err.contains("cannot read the pm3 daemon reply"),
        "got: {err}"
    );
}
