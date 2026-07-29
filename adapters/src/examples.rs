use axum::{
    Json,
    extract::{
        Path, State,
        rejection::{JsonRejection, PathRejection},
    },
    http::StatusCode,
    response::{IntoResponse as _, Response},
};
use serde::{Deserialize, Serialize};
use usecases::{
    CreateExampleError, Example, ExampleStore, StoreError, create_example, find_example,
};

const STORE_UNAVAILABLE_MESSAGE: &str = "example store unavailable";
const NOT_FOUND_MESSAGE: &str = "cannot find example";

type CreateRequest = Result<Json<CreateExampleRequest>, JsonRejection>;
type FindPath = Result<Path<i64>, PathRejection>;

#[derive(Deserialize)]
pub struct CreateExampleRequest {
    pub name: String,
}

#[derive(Serialize)]
pub struct ExampleResponse {
    pub id: i64,
    pub name: String,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

pub async fn create<S>(State(store): State<S>, request: CreateRequest) -> Response
where
    S: ExampleStore + Clone + 'static,
{
    let (response, name) = match request {
        Err(rejection) => (
            malformed_request(rejection.status(), &rejection.body_text()),
            String::new(),
        ),
        Ok(Json(body)) => {
            let CreateExampleRequest { name } = body;
            let response = match create_example(&store, &name).await {
                Ok(example) => {
                    (StatusCode::CREATED, Json(ExampleResponse::from(example))).into_response()
                }
                Err(CreateExampleError::Validation(e)) => {
                    error_response(StatusCode::BAD_REQUEST, &e.to_string())
                }
                Err(CreateExampleError::Store(e)) => store_unavailable(&e),
            };
            (response, name)
        }
    };
    log_request("create", &name, response.status());
    response
}

pub async fn find<S>(State(store): State<S>, path: FindPath) -> Response
where
    S: ExampleStore + Clone + 'static,
{
    let (response, id) = match path {
        Err(rejection) => (
            malformed_request(rejection.status(), &rejection.body_text()),
            String::new(),
        ),
        Ok(Path(id)) => {
            let response = match find_example(&store, id).await {
                Ok(Some(example)) => Json(ExampleResponse::from(example)).into_response(),
                Ok(None) => error_response(StatusCode::NOT_FOUND, NOT_FOUND_MESSAGE),
                Err(e) => store_unavailable(&e),
            };
            (response, id.to_string())
        }
    };
    log_request("find", &id, response.status());
    response
}

impl From<Example> for ExampleResponse {
    fn from(example: Example) -> Self {
        let Example { id, name } = example;
        Self { id, name }
    }
}

fn error_response(status: StatusCode, message: &str) -> Response {
    (
        status,
        Json(ErrorResponse {
            error: message.to_string(),
        }),
    )
        .into_response()
}

fn malformed_request(status: StatusCode, reason: &str) -> Response {
    let code = status.as_u16();
    tracing::debug!(
        feature = "example",
        operation = "request.decode",
        result = "error",
        status = code,
        reason,
        "cannot decode request",
    );
    error_response(status, reason)
}

fn store_unavailable(error: &StoreError) -> Response {
    tracing::warn!(
        feature = "example",
        operation = "store",
        result = "error",
        error = %error,
        "cannot reach example store",
    );
    error_response(StatusCode::SERVICE_UNAVAILABLE, STORE_UNAVAILABLE_MESSAGE)
}

fn log_request(operation: &'static str, req: &str, status: StatusCode) {
    let code = status.as_u16();
    let result = if status.is_success() { "ok" } else { "error" };
    tracing::debug!(
        feature = "example",
        operation,
        result,
        status = code,
        req,
        "example request handled",
    );
}

#[cfg(test)]
#[path = "test_helpers/examples_test_helpers.rs"]
mod test_helpers;
#[cfg(test)]
#[path = "tests/examples_tests.rs"]
mod tests;
