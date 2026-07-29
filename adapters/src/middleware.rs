use std::time::Instant;

use axum::{extract::Request, http::HeaderValue, middleware::Next, response::Response};
use tracing::Instrument as _;

use crate::elapsed_ms;

pub(crate) const REQUEST_ID_HEADER: &str = "x-request-id";
pub(crate) const REQUEST_SPAN_NAME: &str = "http.request";

const RESULT_SERVER_ERROR: &str = "server_error";
const RESULT_CLIENT_ERROR: &str = "client_error";
const RESULT_OK: &str = "ok";

macro_rules! request_handled {
    ($level:ident, $result:expr, $status:expr, $duration_ms:expr) => {
        tracing::$level!(
            feature = "http",
            operation = "request",
            result = $result,
            status = $status,
            duration_ms = $duration_ms,
            "request handled",
        )
    };
}

pub async fn request_id(mut req: Request, next: Next) -> Response {
    #[expect(
        clippy::option_if_let_else,
        reason = "map_or_else closure 形态被 llvm-cov 当作独立未覆盖 fn；显式 match 才能保证函数级覆盖"
    )]
    let request_id = match req
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|v| v.to_str().ok())
    {
        Some(s) => s.to_string(),
        None => uuid::Uuid::new_v4().to_string(),
    };

    let header_val = HeaderValue::from_str(&request_id).expect(
        "internal error: request_id is either a fresh UUID or a visible-ASCII header value",
    );

    req.headers_mut()
        .insert(REQUEST_ID_HEADER, header_val.clone());

    let downstream_span = tracing::info_span!(
        REQUEST_SPAN_NAME,
        request_id = %request_id,
        method = %req.method(),
        path = %req.uri().path(),
    );

    let mut response = run_logged(req, next).instrument(downstream_span).await;
    response.headers_mut().insert(REQUEST_ID_HEADER, header_val);

    response
}

async fn run_logged(req: Request, next: Next) -> Response {
    let started = Instant::now();
    let response = next.run(req).await;
    let duration_ms = elapsed_ms(started);

    let status = response.status().as_u16();
    let result = classify_status_result(status);
    if result == RESULT_SERVER_ERROR {
        request_handled!(warn, result, status, duration_ms);
    } else {
        request_handled!(debug, result, status, duration_ms);
    }

    response
}

fn classify_status_result(status: u16) -> &'static str {
    if (500..=599).contains(&status) {
        RESULT_SERVER_ERROR
    } else if (400..=499).contains(&status) {
        RESULT_CLIENT_ERROR
    } else {
        RESULT_OK
    }
}

#[cfg(test)]
#[path = "test_helpers/middleware_test_helpers.rs"]
mod test_helpers;
#[cfg(test)]
#[path = "tests/middleware_tests.rs"]
mod tests;
