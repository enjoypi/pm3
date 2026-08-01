use std::sync::atomic::{AtomicU64, Ordering};

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use usecases::{AppSelector, UsecaseError};

use super::dto::{HealthDto, ReplyDto, StartRequestDto};
use crate::{
    presenter::{affected_service, already_running_names, refused_names, render_reply},
    state::{DaemonError, DaemonFailure, DaemonHandle, DaemonReply, DaemonRequest},
};

#[allow(clippy::unused_async, reason = "axum Handler trait requires a future")]
pub async fn health() -> Json<HealthDto> {
    Json(HealthDto::healthy())
}

pub async fn start(
    State(handle): State<DaemonHandle>,
    Json(body): Json<StartRequestDto>,
) -> Response {
    let request = DaemonRequest::Start {
        services: body.services,
    };
    dispatch(&handle, request).await
}

pub async fn list(State(handle): State<DaemonHandle>) -> Response {
    dispatch(&handle, DaemonRequest::List).await
}

pub async fn describe(State(handle): State<DaemonHandle>, Path(raw): Path<String>) -> Response {
    dispatch(&handle, DaemonRequest::Describe(selector(&raw))).await
}

pub async fn stop(State(handle): State<DaemonHandle>, Path(raw): Path<String>) -> Response {
    dispatch(&handle, DaemonRequest::Stop(selector(&raw))).await
}

pub async fn restart(State(handle): State<DaemonHandle>, Path(raw): Path<String>) -> Response {
    dispatch(&handle, DaemonRequest::Restart(selector(&raw))).await
}

pub async fn delete(State(handle): State<DaemonHandle>, Path(raw): Path<String>) -> Response {
    dispatch(&handle, DaemonRequest::Delete(selector(&raw))).await
}

pub async fn stop_all(State(handle): State<DaemonHandle>) -> Response {
    dispatch(&handle, DaemonRequest::StopAll).await
}

fn selector(raw: &str) -> AppSelector {
    AppSelector::parse(raw)
}

async fn dispatch(handle: &DaemonHandle, request: DaemonRequest) -> Response {
    let request_id = next_request_id();
    let action = action_of(&request);
    let target = requested_target(&request);
    log_request(request_id, action, &target);
    let outcome = handle.send(request).await;
    let (status, body) = render(outcome);
    log_response(request_id, action, &target, status.as_u16(), &body.report);
    match status {
        StatusCode::OK => (status, Json(body)).into_response(),
        refused => (refused, body.report).into_response(),
    }
}

fn render(outcome: Result<DaemonReply, DaemonError>) -> (StatusCode, ReplyDto) {
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
    }
}

fn next_request_id() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

const fn action_of(request: &DaemonRequest) -> &'static str {
    match request {
        DaemonRequest::Start { .. } => "start",
        DaemonRequest::List => "list",
        DaemonRequest::Describe(_) => "describe",
        DaemonRequest::Stop(_) => "stop",
        DaemonRequest::Restart(_) => "restart",
        DaemonRequest::Delete(_) => "delete",
        DaemonRequest::StopAll => "stop_all",
    }
}

fn requested_target(request: &DaemonRequest) -> String {
    match request {
        DaemonRequest::Start { services } => services.join(","),
        DaemonRequest::List | DaemonRequest::StopAll => String::new(),
        DaemonRequest::Describe(selector)
        | DaemonRequest::Stop(selector)
        | DaemonRequest::Restart(selector)
        | DaemonRequest::Delete(selector) => selector.to_string(),
    }
}

fn log_request(request_id: u64, action: &str, req: &str) {
    tracing::debug!(
        feature = "api",
        request_id,
        action,
        req,
        "the pm3 daemon accepted a request",
    );
}

fn log_response(request_id: u64, action: &str, req: &str, status: u16, resp: &str) {
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

fn envelope(reply: &DaemonReply) -> ReplyDto {
    ReplyDto {
        report: render_reply(reply),
        service: affected_service(reply),
        already_running: already_running_names(reply),
        refused: refused_names(reply),
    }
}

const fn status_of(error: &DaemonError) -> StatusCode {
    match error {
        DaemonError::Unavailable | DaemonError::Dropped => StatusCode::SERVICE_UNAVAILABLE,
        DaemonError::Failed(DaemonFailure::Apps(_)) => StatusCode::BAD_REQUEST,
        DaemonError::Failed(DaemonFailure::Usecase(usecase)) => usecase_status(usecase),
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
