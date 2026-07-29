use axum::{Json, extract::State, http::StatusCode};
use serde::Serialize;

use crate::AppState;

#[cfg(has_database)]
const READINESS_RESULT_ERROR: &str = "error";
#[cfg(has_database)]
const READINESS_RESULT_OK: &str = "ok";
const READINESS_RESULT_DISABLED: &str = "disabled";
#[cfg(not(has_database))]
const DB_LABEL_DISABLED: &str = "disabled";
#[cfg(has_database)]
const DB_LABEL_NOT_CONFIGURED: &str = "not configured";

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
}

#[derive(Serialize)]
pub struct ReadinessResponse {
    pub config: &'static str,
    pub database: serde_json::Value,
}

#[expect(clippy::unused_async, reason = "axum handler signature requires async")]
pub async fn health() -> Json<HealthResponse> {
    tracing::debug!(
        feature = "health",
        operation = "liveness",
        result = "ok",
        "liveness probe",
    );
    Json(HealthResponse { status: "ok" })
}

pub async fn readiness(State(state): State<AppState>) -> (StatusCode, Json<ReadinessResponse>) {
    let readiness = db_readiness(&state).await;
    let result = readiness.result();
    let status_code = readiness_status_code(&readiness);
    let database = readiness.into_body();
    tracing::debug!(
        feature = "health",
        operation = "readiness",
        result,
        db_status = %database,
        "readiness probe",
    );
    (
        status_code,
        Json(ReadinessResponse {
            config: "loaded",
            database,
        }),
    )
}

enum DbReadiness {
    #[cfg(has_database)]
    Connected(serde_json::Value),
    #[cfg(has_database)]
    Failed(serde_json::Value),
    Inactive(&'static str),
}

impl DbReadiness {
    const fn result(&self) -> &'static str {
        match self {
            #[cfg(has_database)]
            Self::Connected(_) => READINESS_RESULT_OK,
            #[cfg(has_database)]
            Self::Failed(_) => READINESS_RESULT_ERROR,
            Self::Inactive(_) => READINESS_RESULT_DISABLED,
        }
    }

    fn into_body(self) -> serde_json::Value {
        match self {
            #[cfg(has_database)]
            Self::Connected(body) | Self::Failed(body) => body,
            Self::Inactive(label) => serde_json::json!(label),
        }
    }
}

#[cfg(has_database)]
const fn readiness_status_code(readiness: &DbReadiness) -> StatusCode {
    match readiness {
        DbReadiness::Failed(_) => StatusCode::SERVICE_UNAVAILABLE,
        DbReadiness::Connected(_) | DbReadiness::Inactive(_) => StatusCode::OK,
    }
}

#[cfg(not(has_database))]
const fn readiness_status_code(readiness: &DbReadiness) -> StatusCode {
    match readiness {
        DbReadiness::Inactive(_) => StatusCode::OK,
    }
}

#[cfg(has_database)]
async fn db_readiness(state: &AppState) -> DbReadiness {
    let Some(db) = &state.db else {
        return DbReadiness::Inactive(DB_LABEL_NOT_CONFIGURED);
    };
    let health =
        crate::database::check_health(&db.pool, db.health_check_timeout, db.expected_migrations)
            .await;
    let health_state = health.state;
    let body = serde_json::to_value(health)
        .expect("internal error: DatabaseHealthStatus has only String/numeric fields");
    match health_state {
        crate::database::DbHealthState::Connected => DbReadiness::Connected(body),
        crate::database::DbHealthState::Failed
        | crate::database::DbHealthState::MigrationsPending => DbReadiness::Failed(body),
    }
}

#[cfg(not(has_database))]
#[expect(
    clippy::unused_async,
    reason = "签名需与 has_database 版本对齐让 readiness handler 调用点无需 cfg 分叉"
)]
async fn db_readiness(_state: &AppState) -> DbReadiness {
    DbReadiness::Inactive(DB_LABEL_DISABLED)
}

#[cfg(test)]
#[path = "tests/handlers_tests.rs"]
mod tests;
