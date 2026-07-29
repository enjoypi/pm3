pub async fn body_json(body: axum::body::Body) -> serde_json::Value {
    let bytes = axum::body::to_bytes(body, usize::MAX)
        .await
        .expect("read response body");
    serde_json::from_slice(&bytes).expect("response body is JSON")
}
