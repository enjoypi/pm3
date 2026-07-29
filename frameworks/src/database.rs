use std::{
    path::Path,
    time::{Duration, Instant},
};

use adapters::{
    DatabaseConfig,
    database::{DatabaseHealthStatus, DbPool, MigrationCounts},
};
use sqlx::migrate::Migrator;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("cannot connect to database: {0}")]
    ConnectionFailed(#[source] sqlx::Error),

    #[error("cannot accept database URL scheme {actual}: expected one of {expected}")]
    SchemeMismatch { expected: String, actual: String },

    #[error("cannot find migrations directory {0}")]
    MigrationsDirectoryNotFound(String),

    #[error("cannot run migrations: {0}")]
    MigrationFailed(#[source] sqlx::migrate::MigrateError),

    #[error("cannot execute query: {0}")]
    QueryFailed(#[source] sqlx::Error),

    #[error("cannot create parent directory for SQLite: {0}")]
    SqliteParentDir(#[source] std::io::Error),
}

const fn expected_schemes() -> &'static [&'static str] {
    #[cfg(feature = "sqlite")]
    {
        &["sqlite"]
    }
    #[cfg(feature = "postgres")]
    {
        &["postgres", "postgresql"]
    }
    #[cfg(feature = "mysql")]
    {
        &["mysql", "mariadb"]
    }
}

fn validate_url_scheme(url: &str) -> Result<(), DatabaseError> {
    let schemes = expected_schemes();
    let actual = url.split(':').next().unwrap_or("unknown");

    if schemes.contains(&actual) {
        return Ok(());
    }

    Err(DatabaseError::SchemeMismatch {
        expected: schemes.join(" | "),
        actual: actual.to_string(),
    })
}

pub async fn create_pool(config: &DatabaseConfig) -> Result<DbPool, DatabaseError> {
    validate_url_scheme(&config.url)?;

    #[cfg(feature = "sqlite")]
    ensure_sqlite_parent_dir(&config.url)?;

    let redacted = adapters::config::redact_url(&config.url);
    let started = Instant::now();
    let pool = sqlx::pool::PoolOptions::new()
        .max_connections(config.pool.max_connections)
        .min_connections(config.pool.min_connections)
        .acquire_timeout(Duration::from_secs(config.pool.acquire_timeout_secs))
        .idle_timeout(Duration::from_secs(config.pool.idle_timeout_secs))
        .max_lifetime(Duration::from_secs(config.pool.max_lifetime_secs))
        .connect(&config.url)
        .await
        .inspect_err(|e| log_pool_create_failure(&redacted, adapters::elapsed_ms(started), e))
        .map_err(DatabaseError::ConnectionFailed)?;

    let duration_ms = adapters::elapsed_ms(started);
    tracing::info!(
        feature = "database",
        operation = "pool.create",
        result = "ok",
        duration_ms,
        url = %redacted,
        max_connections = config.pool.max_connections,
        "database pool created",
    );
    Ok(pool)
}

fn log_pool_create_failure(redacted_url: &str, duration_ms: u64, error: &sqlx::Error) {
    tracing::error!(
        feature = "database",
        operation = "pool.create",
        result = "error",
        duration_ms,
        url = %redacted_url,
        error = %error,
        "cannot create database pool",
    );
}

async fn load_migrator(migrations_path: &Path) -> Result<Migrator, DatabaseError> {
    if !migrations_path.exists() {
        return Err(DatabaseError::MigrationsDirectoryNotFound(
            migrations_path.display().to_string(),
        ));
    }
    Migrator::new(migrations_path)
        .await
        .map_err(DatabaseError::MigrationFailed)
}

pub async fn run_migrations(pool: &DbPool, migrations_path: &Path) -> Result<(), DatabaseError> {
    let migrator = load_migrator(migrations_path).await?;
    let total = migrator.iter().count();

    tracing::debug!(
        feature = "database",
        operation = "migrate.start",
        result = "ok",
        total,
        "starting migrations",
    );

    let started = Instant::now();
    migrator
        .run(pool)
        .await
        .inspect_err(|e| log_migration_failure(total, adapters::elapsed_ms(started), e))
        .map_err(DatabaseError::MigrationFailed)?;

    let duration_ms = adapters::elapsed_ms(started);
    tracing::info!(
        feature = "database",
        operation = "migrate.run",
        result = "ok",
        duration_ms,
        total,
        "migrations applied",
    );

    Ok(())
}

fn log_migration_failure(total: usize, duration_ms: u64, error: &sqlx::migrate::MigrateError) {
    tracing::error!(
        feature = "database",
        operation = "migrate.run",
        result = "error",
        duration_ms,
        total,
        error = %error,
        "cannot run migrations",
    );
}

pub async fn expected_migration_count(migrations_path: &Path) -> Result<i64, DatabaseError> {
    let migrator = load_migrator(migrations_path).await?;
    Ok(migrator_total(&migrator))
}

fn migrator_total(migrator: &Migrator) -> i64 {
    i64::try_from(migrator.iter().count())
        .expect("internal error: migration count fits in i64 on supported platforms")
}

pub async fn migration_counts(
    pool: &DbPool,
    migrations_path: &Path,
) -> Result<MigrationCounts, DatabaseError> {
    let migrator = load_migrator(migrations_path).await?;
    let total = migrator_total(&migrator);

    let applied: i64 = adapters::database::count_applied_migrations(pool)
        .await
        .map_err(DatabaseError::QueryFailed)?;

    Ok(MigrationCounts { applied, total })
}

pub async fn db_status_snapshot(
    pool: &DbPool,
    migrations_path: &Path,
    health_check_timeout: Duration,
) -> (DatabaseHealthStatus, Result<MigrationCounts, DatabaseError>) {
    let counts = migration_counts(pool, migrations_path).await;
    let expected = counts.as_ref().map_or(0, |counts| counts.total);
    let health = adapters::database::check_health(pool, health_check_timeout, expected).await;
    (health, counts)
}

#[cfg(feature = "sqlite")]
fn ensure_sqlite_parent_dir(url: &str) -> Result<(), DatabaseError> {
    let path_str = url
        .strip_prefix("sqlite://")
        .or_else(|| url.strip_prefix("sqlite:"))
        .unwrap_or(url);

    let path_str = path_str.split('?').next().unwrap_or(path_str);

    if let Some(parent) = Path::new(path_str).parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(DatabaseError::SqliteParentDir)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "test_helpers/database_test_helpers.rs"]
mod test_helpers;
#[cfg(test)]
#[path = "tests/database_tests.rs"]
mod tests;
