use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use usecases::{AppSelector, UsecaseError};

use super::dto::{HealthDto, ReplyDto, StartRequestDto};
use crate::{
    presenter::{affected_service, already_running_names, render_reply},
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
    respond(handle.send(request).await)
}

pub async fn list(State(handle): State<DaemonHandle>) -> Response {
    respond(handle.send(DaemonRequest::List).await)
}

pub async fn describe(State(handle): State<DaemonHandle>, Path(raw): Path<String>) -> Response {
    respond(handle.send(DaemonRequest::Describe(selector(&raw))).await)
}

pub async fn stop(State(handle): State<DaemonHandle>, Path(raw): Path<String>) -> Response {
    respond(handle.send(DaemonRequest::Stop(selector(&raw))).await)
}

pub async fn restart(State(handle): State<DaemonHandle>, Path(raw): Path<String>) -> Response {
    respond(handle.send(DaemonRequest::Restart(selector(&raw))).await)
}

pub async fn delete(State(handle): State<DaemonHandle>, Path(raw): Path<String>) -> Response {
    respond(handle.send(DaemonRequest::Delete(selector(&raw))).await)
}

pub async fn stop_all(State(handle): State<DaemonHandle>) -> Response {
    respond(handle.send(DaemonRequest::StopAll).await)
}

fn selector(raw: &str) -> AppSelector {
    AppSelector::parse(raw)
}

fn respond(outcome: Result<DaemonReply, DaemonError>) -> Response {
    match outcome {
        Ok(reply) => (StatusCode::OK, Json(envelope(&reply))).into_response(),
        Err(error) => (status_of(&error), error.to_string()).into_response(),
    }
}

fn envelope(reply: &DaemonReply) -> ReplyDto {
    ReplyDto {
        report: render_reply(reply),
        service: affected_service(reply),
        already_running: already_running_names(reply),
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
