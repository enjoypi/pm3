pub async fn body_json(body: axum::body::Body) -> serde_json::Value {
    let bytes = axum::body::to_bytes(body, usize::MAX)
        .await
        .expect("read response body");
    serde_json::from_slice(&bytes).expect("response body is JSON")
}

pub async fn body_text(body: axum::body::Body) -> String {
    let bytes = axum::body::to_bytes(body, usize::MAX)
        .await
        .expect("read response body");
    String::from_utf8(bytes.to_vec()).expect("response body is UTF-8")
}
