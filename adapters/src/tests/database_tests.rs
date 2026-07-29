use super::*;

#[tokio::test]
async fn check_health_connected() {
    let (pool, _dir) = test_pool_in_tempdir().await;

    let health = check_health(&pool, Duration::from_secs(3), 0).await;
    assert_eq!(health.state, DbHealthState::Connected);
    assert_eq!(health.status, "connected");
    assert!(health.pool.is_some());

    pool.close().await;
}

#[tokio::test]
async fn check_health_query_error_after_close() {
    let (pool, _dir) = test_pool_in_tempdir().await;
    pool.close().await;
    let health = check_health(&pool, Duration::from_secs(3), 0).await;
    assert_eq!(health.state, DbHealthState::Failed);
    assert!(health.pool.is_none());
}

#[tokio::test]
async fn check_health_timeout() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let url = db_paths::sqlite_rwc_url(&dir.path().join("timeout.db"));
    let pool: DbPool = sqlx::pool::PoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .expect("create pool");
    let _hold = pool.acquire().await.expect("hold the only connection");

    let health = check_health(&pool, Duration::from_millis(50), 0).await;
    assert_eq!(health.state, DbHealthState::Failed);
    assert_eq!(health.status, "error: health check timed out");
    assert!(health.pool.is_none());
}

#[tokio::test]
async fn check_health_reports_pending_migrations_as_not_ready() {
    let (pool, _dir) = test_pool_in_tempdir().await;

    let health = check_health(&pool, Duration::from_secs(3), 2).await;

    assert_eq!(health.state, DbHealthState::MigrationsPending);
    pool.close().await;
}

#[tokio::test]
async fn check_health_counts_pending_migrations_in_body() {
    let (pool, _dir) = test_pool_in_tempdir().await;

    let health = check_health(&pool, Duration::from_secs(3), 2).await;

    assert_eq!(
        health.migrations,
        Some(MigrationReadiness {
            applied: 0,
            pending: 2
        })
    );
    pool.close().await;
}

#[tokio::test]
async fn check_health_is_ready_once_every_migration_is_applied() {
    let (pool, _dir) = test_pool_in_tempdir().await;
    sqlx::migrate::Migrator::new(workspace_migrations_dir().as_path())
        .await
        .expect("load migrator")
        .run(&pool)
        .await
        .expect("run migrations");

    let health = check_health(&pool, Duration::from_secs(3), 1).await;

    assert_eq!(
        health.state,
        DbHealthState::Connected,
        "got status: {}",
        health.status
    );
    pool.close().await;
}

#[tokio::test]
async fn count_applied_migrations_no_table() {
    let (pool, _dir) = test_pool_in_tempdir().await;
    let result = count_applied_migrations(&pool).await;
    assert!(result.is_err());
    pool.close().await;
}

#[test]
fn migration_counts_pending_is_total_minus_applied() {
    let counts = MigrationCounts {
        applied: 1,
        total: 3,
    };
    assert_eq!(counts.pending(), 2);
}

#[test]
fn migration_counts_pending_clamps_to_zero_when_applied_exceeds_total() {
    let counts = MigrationCounts {
        applied: 100,
        total: 10,
    };
    assert_eq!(counts.pending(), 0);
}

#[test]
fn format_migration_status_renders_three_counts() {
    let s = format_migration_status(MigrationCounts {
        applied: 2,
        total: 3,
    });
    assert_eq!(s, "Migrations: 2 applied, 1 pending (of 3 total)");
}

#[test]
fn format_db_status_report_with_pool() {
    let health = DatabaseHealthStatus {
        status: "connected".to_string(),
        migrations: None,
        pool: Some(PoolStatus {
            size: 5,
            idle: 3,
            max: 10,
        }),
        state: DbHealthState::Connected,
    };
    let counts = MigrationCounts {
        applied: 1,
        total: 1,
    };
    let s = format_db_status_report(&health, Ok(counts));
    assert!(s.contains("Database: connected"));
    assert!(s.contains("Pool: 2 active, 3 idle, 10 max"));
    assert!(s.contains("Migrations: 1 applied, 0 pending (of 1 total)"));
}

#[test]
fn format_db_status_report_without_pool() {
    let health = DatabaseHealthStatus {
        status: "error: down".to_string(),
        migrations: None,
        pool: None,
        state: DbHealthState::Failed,
    };
    let counts = MigrationCounts {
        applied: 0,
        total: 0,
    };
    let s = format_db_status_report(&health, Ok(counts));
    assert!(s.contains("Database: error: down"));
    assert!(!s.contains("Pool:"));
}

#[test]
fn format_db_status_report_with_migrations_error_preserves_health_line() {
    let health = DatabaseHealthStatus {
        status: "connected".to_string(),
        migrations: None,
        pool: Some(PoolStatus {
            size: 2,
            idle: 1,
            max: 5,
        }),
        state: DbHealthState::Connected,
    };
    let s = format_db_status_report(&health, Err("cannot find migrations directory /missing"));
    assert!(s.contains("Database: connected"), "got: {s}");
    assert!(s.contains("Pool: 1 active, 1 idle, 5 max"), "got: {s}");
    assert!(
        s.contains("Migrations: error: cannot find migrations directory /missing"),
        "got: {s}"
    );
}

#[test]
fn format_db_status_report_saturates_when_idle_exceeds_size() {
    let health = DatabaseHealthStatus {
        status: "connected".to_string(),
        migrations: None,
        pool: Some(PoolStatus {
            size: 1,
            idle: 5,
            max: 10,
        }),
        state: DbHealthState::Connected,
    };
    let counts = MigrationCounts {
        applied: 0,
        total: 0,
    };
    let s = format_db_status_report(&health, Ok(counts));
    assert!(s.contains("Pool: 0 active, 5 idle, 10 max"), "got: {s}");
}

#[test]
fn migrations_applied_message_constant() {
    assert_eq!(
        MIGRATIONS_APPLIED_MESSAGE,
        "Migrations applied successfully"
    );
}

#[tokio::test]
async fn count_applied_migrations_after_migrate() {
    let (pool, _dir) = test_pool_in_tempdir().await;
    let migrations_dir = workspace_migrations_dir();
    sqlx::migrate::Migrator::new(migrations_dir.as_path())
        .await
        .expect("load migrator")
        .run(&pool)
        .await
        .expect("run migrations");
    let count = count_applied_migrations(&pool).await.expect("count");
    assert!(
        count >= 1,
        "expected at least 1 applied migration, got {count}"
    );
    pool.close().await;
}
