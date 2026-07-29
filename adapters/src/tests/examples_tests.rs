use super::{test_helpers::*, *};

fn create_body(name: &str) -> Json<CreateExampleRequest> {
    Json(CreateExampleRequest {
        name: name.to_string(),
    })
}

#[tokio::test]
async fn create_returns_created_with_persisted_example() {
    let response = create(State(MockStore::working()), Ok(create_body("widget"))).await;

    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn create_echoes_submitted_name_in_body() {
    let response = create(State(MockStore::working()), Ok(create_body("widget"))).await;

    let body = body_json(response.into_body()).await;
    assert_eq!(body["name"], "widget", "got: {body}");
}

#[tokio::test]
async fn create_rejects_empty_name_as_bad_request() {
    let response = create(State(MockStore::working()), Ok(create_body("   "))).await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_maps_store_failure_to_service_unavailable() {
    let response = create(State(MockStore::unreachable()), Ok(create_body("widget"))).await;

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn create_hides_store_error_detail_from_client() {
    let response = create(State(MockStore::unreachable()), Ok(create_body("widget"))).await;

    let body = body_json(response.into_body()).await;
    assert_eq!(body["error"], STORE_UNAVAILABLE_MESSAGE, "got: {body}");
}

async fn json_rejection(body: &'static str) -> JsonRejection {
    use axum::extract::FromRequest as _;

    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/examples")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body))
        .expect("request");
    let Err(rejection) = Json::<CreateExampleRequest>::from_request(request, &()).await else {
        panic!("a malformed body must be rejected by the extractor");
    };
    rejection
}

async fn path_rejection() -> PathRejection {
    use axum::extract::FromRequestParts as _;

    let (mut parts, _body) = axum::http::Request::builder()
        .uri("/examples/abc")
        .body(axum::body::Body::empty())
        .expect("request")
        .into_parts();
    Path::<i64>::from_request_parts(&mut parts, &())
        .await
        .expect_err("a non-numeric id must be rejected by the extractor")
}

#[tokio::test]
async fn create_reports_body_rejection_as_json_error() {
    let rejection = json_rejection("{not json").await;

    let response = create(State(MockStore::working()), Err(rejection)).await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_json(response.into_body()).await;
    assert!(
        body["error"].is_string(),
        "a Json rejection must keep the ErrorResponse contract, got: {body}"
    );
}

#[tokio::test]
async fn find_reports_path_rejection_as_json_error() {
    let rejection = path_rejection().await;

    let response = find(State(MockStore::working()), Err(rejection)).await;

    let body = body_json(response.into_body()).await;
    assert!(
        body["error"].is_string(),
        "a Path rejection must keep the ErrorResponse contract, got: {body}"
    );
}

#[tokio::test]
async fn find_returns_ok_for_existing_example() {
    let response = find(State(MockStore::working()), Ok(Path(FOUND_ID))).await;

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn find_returns_stored_name_in_body() {
    let response = find(State(MockStore::working()), Ok(Path(FOUND_ID))).await;

    let body = body_json(response.into_body()).await;
    assert_eq!(body["name"], FOUND_NAME, "got: {body}");
}

#[tokio::test]
async fn find_returns_not_found_for_missing_example() {
    let response = find(State(MockStore::working()), Ok(Path(404))).await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn find_maps_store_failure_to_service_unavailable() {
    let response = find(State(MockStore::unreachable()), Ok(Path(FOUND_ID))).await;

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}
