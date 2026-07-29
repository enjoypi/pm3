use std::time::{Duration, Instant};

use serde::Serialize;

#[cfg(all(feature = "sqlite", feature = "postgres"))]
compile_error!("Only one database feature can be enabled. Choose one of: sqlite, postgres, mysql.");

#[cfg(all(feature = "sqlite", feature = "mysql"))]
compile_error!("Only one database feature can be enabled. Choose one of: sqlite, postgres, mysql.");

#[cfg(all(feature = "postgres", feature = "mysql"))]
compile_error!("Only one database feature can be enabled. Choose one of: sqlite, postgres, mysql.");

#[cfg(feature = "sqlite")]
pub type DbPool = sqlx::SqlitePool;

#[cfg(feature = "postgres")]
pub type DbPool = sqlx::PgPool;

#[cfg(feature = "mysql")]
pub type DbPool = sqlx::MySqlPool;

const STATUS_CONNECTED: &str = "connected";
const STATUS_TIMED_OUT: &str = "error: health check timed out";

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum DbHealthState {
    Connected,
    Failed,
    MigrationsPending,
}

#[derive(Debug, Serialize)]
pub struct DatabaseHealthStatus {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub migrations: Option<MigrationReadiness>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pool: Option<PoolStatus>,
    #[serde(skip)]
    pub state: DbHealthState,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MigrationReadiness {
    pub applied: i64,
    pub pending: i64,
}

#[derive(Debug, Serialize)]
pub struct PoolStatus {
    pub size: u32,
    pub idle: u32,
    pub max: u32,
}

pub async fn check_health(
    pool: &DbPool,
    health_check_timeout: Duration,
    expected_migrations: i64,
) -> DatabaseHealthStatus {
    let started = Instant::now();
    let probed = tokio::time::timeout(
        health_check_timeout,
        probe_readiness(pool, expected_migrations),
    )
    .await;
    let duration_ms = crate::elapsed_ms(started);

    match probed {
        Ok(Ok(migrations)) => reachable_status(pool, migrations, duration_ms),
        Ok(Err(e)) => failed_status(&e, duration_ms),
        Err(_) => timed_out_status(health_check_timeout, duration_ms),
    }
}

async fn probe_readiness(
    pool: &DbPool,
    expected_migrations: i64,
) -> Result<MigrationReadiness, sqlx::Error> {
    sqlx::query("SELECT 1").execute(pool).await?;
    let applied = applied_migration_count(pool).await;
    Ok(MigrationReadiness {
        applied,
        pending: (expected_migrations - applied).max(0),
    })
}

async fn applied_migration_count(pool: &DbPool) -> i64 {
    match count_applied_migrations(pool).await {
        Ok(applied) => applied,
        Err(e) => {
            tracing::warn!(
                feature = "database",
                operation = "migrations.count",
                result = "error",
                error = %e,
                "cannot count applied migrations, treating as none applied",
            );
            0
        }
    }
}

fn reachable_status(
    pool: &DbPool,
    migrations: MigrationReadiness,
    duration_ms: u64,
) -> DatabaseHealthStatus {
    let MigrationReadiness {
        applied: _,
        pending,
    } = migrations;
    let state = if pending > 0 {
        DbHealthState::MigrationsPending
    } else {
        DbHealthState::Connected
    };
    tracing::debug!(
        feature = "database",
        operation = "health.check",
        result = "ok",
        duration_ms,
        pending_migrations = pending,
        "db health ok",
    );
    DatabaseHealthStatus {
        status: STATUS_CONNECTED.to_string(),
        migrations: Some(migrations),
        pool: Some(PoolStatus {
            size: pool.size(),
            idle: u32::try_from(pool.num_idle()).unwrap_or(0),
            max: pool.options().get_max_connections(),
        }),
        state,
    }
}

fn failed_status(error: &sqlx::Error, duration_ms: u64) -> DatabaseHealthStatus {
    tracing::warn!(
        feature = "database",
        operation = "health.check",
        result = "error",
        duration_ms,
        error = %error,
        "db health query failed",
    );
    DatabaseHealthStatus {
        status: format!("error: {error}"),
        migrations: None,
        pool: None,
        state: DbHealthState::Failed,
    }
}

fn timed_out_status(health_check_timeout: Duration, duration_ms: u64) -> DatabaseHealthStatus {
    let timeout_secs = health_check_timeout.as_secs();
    tracing::warn!(
        feature = "database",
        operation = "health.check",
        result = "timeout",
        duration_ms,
        timeout_secs,
        "db health timed out",
    );
    DatabaseHealthStatus {
        status: STATUS_TIMED_OUT.to_string(),
        migrations: None,
        pool: None,
        state: DbHealthState::Failed,
    }
}

pub async fn count_applied_migrations(pool: &DbPool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(pool)
        .await
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct MigrationCounts {
    pub applied: i64,
    pub total: i64,
}

impl MigrationCounts {
    #[must_use]
    pub fn pending(&self) -> i64 {
        (self.total - self.applied).max(0)
    }
}

pub const MIGRATIONS_APPLIED_MESSAGE: &str = "Migrations applied successfully";

#[must_use]
pub fn format_migration_status(counts: MigrationCounts) -> String {
    format!(
        "Migrations: {} applied, {} pending (of {} total)",
        counts.applied,
        counts.pending(),
        counts.total
    )
}

#[must_use]
pub fn format_db_status_report(
    health: &DatabaseHealthStatus,
    counts: Result<MigrationCounts, &str>,
) -> String {
    let mut lines = vec![format!("Database: {}", health.status)];
    if let Some(pool_status) = &health.pool {
        lines.push(format!(
            "Pool: {} active, {} idle, {} max",
            pool_status.size.saturating_sub(pool_status.idle),
            pool_status.idle,
            pool_status.max,
        ));
    }
    match counts {
        Ok(c) => lines.push(format_migration_status(c)),
        Err(e) => lines.push(format!("Migrations: error: {e}")),
    }
    lines.join("\n")
}

#[cfg(test)]
#[allow(
    dead_code,
    reason = "shared fixture file; consumers per feature combination use a subset"
)]
#[path = "../test_support/db_paths.rs"]
mod db_paths;

#[cfg(test)]
pub(crate) use self::db_paths::workspace_migrations_dir;

#[cfg(test)]
pub(crate) async fn test_pool_in_tempdir() -> (DbPool, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("create temp dir");
    let url = self::db_paths::sqlite_rwc_url(&dir.path().join("test.db"));
    let pool = sqlx::pool::PoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .expect("create test pool");
    (pool, dir)
}

#[cfg(test)]
#[path = "tests/database_tests.rs"]
mod tests;
