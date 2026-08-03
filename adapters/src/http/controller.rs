use std::sync::atomic::{AtomicU64, Ordering};

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use usecases::{
    AppSelector, SupervisionFailure, SupervisionReply, SupervisionRequest, UsecaseError,
};

use super::{
    dto::{HealthDto, ReplyDto, StartRequestDto},
    routes::REQUEST_ID_HEADER,
};
use crate::{
    presenter::{
        affected_service, already_running_names, refused_names, render_reply, unsaved_reason,
    },
    state::{DaemonError, DaemonHandle},
};

#[allow(clippy::unused_async, reason = "axum Handler trait requires a future")]
pub async fn health() -> Json<HealthDto> {
    Json(HealthDto::healthy())
}

pub async fn start(
    State(handle): State<DaemonHandle>,
    headers: HeaderMap,
    Json(body): Json<StartRequestDto>,
) -> Response {
    let request = SupervisionRequest::Start {
        services: body.services,
    };
    dispatch(&handle, &headers, request).await
}

pub async fn list(State(handle): State<DaemonHandle>, headers: HeaderMap) -> Response {
    dispatch(&handle, &headers, SupervisionRequest::List).await
}

pub async fn describe(
    State(handle): State<DaemonHandle>,
    headers: HeaderMap,
    Path(raw): Path<String>,
) -> Response {
    dispatch(
        &handle,
        &headers,
        SupervisionRequest::Describe(selector(&raw)),
    )
    .await
}

pub async fn stop(
    State(handle): State<DaemonHandle>,
    headers: HeaderMap,
    Path(raw): Path<String>,
) -> Response {
    dispatch(&handle, &headers, SupervisionRequest::Stop(selector(&raw))).await
}

pub async fn restart(
    State(handle): State<DaemonHandle>,
    headers: HeaderMap,
    Path(raw): Path<String>,
) -> Response {
    dispatch(
        &handle,
        &headers,
        SupervisionRequest::Restart(selector(&raw)),
    )
    .await
}

pub async fn delete(
    State(handle): State<DaemonHandle>,
    headers: HeaderMap,
    Path(raw): Path<String>,
) -> Response {
    dispatch(
        &handle,
        &headers,
        SupervisionRequest::Delete(selector(&raw)),
    )
    .await
}

pub async fn stop_all(State(handle): State<DaemonHandle>, headers: HeaderMap) -> Response {
    dispatch(&handle, &headers, SupervisionRequest::StopAll).await
}

fn selector(raw: &str) -> AppSelector {
    AppSelector::parse(raw)
}

async fn dispatch(
    handle: &DaemonHandle,
    headers: &HeaderMap,
    request: SupervisionRequest,
) -> Response {
    let request_id = request_id_of(headers);
    let action = request.action();
    let target = request.target();
    log_request(&request_id, action, &target);
    let outcome = handle.send(request).await;
    let (status, body) = render(outcome);
    log_response(&request_id, action, &target, status.as_u16(), &body.report);
    match status {
        StatusCode::OK => (status, Json(body)).into_response(),
        refused => (refused, body.report).into_response(),
    }
}

fn render(outcome: Result<SupervisionReply, DaemonError>) -> (StatusCode, ReplyDto) {
    match outcome {
        Ok(reply) => (StatusCode::OK, envelope(&reply)),
        Err(error) => (status_of(&error), refusal(&error)),
    }
}

fn refusal(error: &DaemonError) -> ReplyDto {
    ReplyDto {
        report: error.to_string(),
        service: None,
        already_running: Vec::new(),
        refused: Vec::new(),
        unsaved: None,
    }
}

pub fn request_id_of(headers: &HeaderMap) -> String {
    headers
        .get(REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty())
        .map_or_else(|| next_request_id().to_string(), ToString::to_string)
}

fn next_request_id() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

fn log_request(request_id: &str, action: &str, req: &str) {
    tracing::debug!(
        feature = "api",
        request_id,
        action,
        req,
        "the pm3 daemon accepted a request",
    );
}

fn log_response(request_id: &str, action: &str, req: &str, status: u16, resp: &str) {
    tracing::debug!(
        feature = "api",
        request_id,
        action,
        req,
        status,
        resp,
        "the pm3 daemon answered a request",
    );
}

fn envelope(reply: &SupervisionReply) -> ReplyDto {
    ReplyDto {
        report: render_reply(reply),
        service: affected_service(reply),
        already_running: already_running_names(reply),
        refused: refused_names(reply),
        unsaved: unsaved_reason(reply),
    }
}

const fn status_of(error: &DaemonError) -> StatusCode {
    match error {
        DaemonError::Unavailable | DaemonError::Dropped => StatusCode::SERVICE_UNAVAILABLE,
        DaemonError::Failed(SupervisionFailure::Spec(_)) => StatusCode::BAD_REQUEST,
        DaemonError::Failed(SupervisionFailure::Usecase(usecase)) => usecase_status(usecase),
    }
}

const fn usecase_status(error: &UsecaseError) -> StatusCode {
    use UsecaseError as Ue;

    match error {
        Ue::NotFound(_) => StatusCode::NOT_FOUND,
        Ue::StillDependedOn { .. } => StatusCode::CONFLICT,
        Ue::Spec(_) | Ue::Dependency(_) | Ue::Policy(_) | Ue::Sandbox(_) => StatusCode::BAD_REQUEST,
        Ue::Launch(_) | Ue::Signal(_) | Ue::Dump(_) | Ue::Fingerprint(_) => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

#[cfg(test)]
#[path = "../test_helpers/http_controller_test_helpers.rs"]
mod test_helpers;
#[cfg(test)]
#[path = "../tests/http_controller_tests.rs"]
mod tests;
