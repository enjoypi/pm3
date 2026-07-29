use super::{test_helpers::*, *};

#[tokio::test]
async fn expected_migration_count_counts_workspace_migrations() {
    let count = expected_migration_count(&crate::test_helpers::workspace_migrations_dir())
        .await
        .expect("count workspace migrations");
    assert!(count >= 1, "expected at least one migration, got {count}");
}

#[tokio::test]
async fn expected_migration_count_missing_directory_returns_error() {
    let err = expected_migration_count(Path::new("/nonexistent/migrations"))
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("cannot find migrations directory"),
        "got: {err}"
    );
}

#[test]
fn validate_url_scheme_correct() {
    #[cfg(feature = "sqlite")]
    assert!(validate_url_scheme("sqlite://test.db").is_ok());
    #[cfg(feature = "postgres")]
    assert!(validate_url_scheme("postgres://localhost/db").is_ok());
    #[cfg(feature = "mysql")]
    assert!(validate_url_scheme("mysql://localhost/db").is_ok());
}

#[test]
fn validate_url_scheme_accepts_sqlx_alias() {
    #[cfg(feature = "sqlite")]
    assert!(validate_url_scheme("sqlite:test.db").is_ok());
    #[cfg(feature = "postgres")]
    assert!(validate_url_scheme("postgresql://localhost/db").is_ok());
    #[cfg(feature = "mysql")]
    assert!(validate_url_scheme("mariadb://localhost/db").is_ok());
}

#[test]
fn validate_url_scheme_mismatch() {
    #[cfg(feature = "sqlite")]
    let wrong_url = "postgres://localhost/db";
    #[cfg(any(feature = "postgres", feature = "mysql"))]
    let wrong_url = "sqlite://test.db";
    let err = validate_url_scheme(wrong_url).unwrap_err();
    assert!(
        err.to_string()
            .contains("cannot accept database URL scheme"),
        "got: {err}"
    );
}

#[cfg(feature = "sqlite")]
#[test]
fn validate_url_scheme_partial_match_rejected() {
    let err = validate_url_scheme("sqlitex://test.db").unwrap_err();
    assert!(
        err.to_string()
            .contains("cannot accept database URL scheme"),
        "got: {err}"
    );
}

#[tokio::test]
async fn create_pool_sqlite_in_tempdir() {
    let (pool, _dir) = test_pool_in_tempdir().await;
    assert!(pool.size() > 0);
    pool.close().await;
}

#[tokio::test]
async fn create_pool_url_scheme_mismatch() {
    #[cfg(feature = "sqlite")]
    let wrong_url = "postgres://localhost/db";
    #[cfg(any(feature = "postgres", feature = "mysql"))]
    let wrong_url = "sqlite://test.db";
    let config = test_db_config(wrong_url);
    let err = create_pool(&config).await.unwrap_err();
    assert!(
        err.to_string()
            .contains("cannot accept database URL scheme"),
        "got: {err}"
    );
}

#[tokio::test]
async fn run_migrations_missing_dir() {
    let (pool, _dir) = test_pool_in_tempdir().await;

    let result = run_migrations(&pool, Path::new("/nonexistent/migrations")).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("cannot find migrations directory"),
        "got: {err}"
    );
    pool.close().await;
}

#[tokio::test]
async fn run_migrations_and_check_status() {
    let (pool, _dir) = test_pool_in_tempdir().await;

    let migrations_dir = crate::test_helpers::workspace_migrations_dir();

    run_migrations(&pool, &migrations_dir)
        .await
        .expect("migration ok");

    let counts = migration_counts(&pool, &migrations_dir)
        .await
        .expect("counts ok");
    assert_eq!(counts.applied, 1);
    assert_eq!(counts.pending(), 0);
    assert_eq!(counts.total, 1);

    pool.close().await;
}

#[cfg(feature = "sqlite")]
#[test]
fn ensure_sqlite_parent_dir_creates_missing() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let nested = dir.path().join("sub/dir/test.db");
    let url = format!("sqlite://{}", nested.display());
    ensure_sqlite_parent_dir(&url).expect("should create parent");
    assert!(dir.path().join("sub/dir").exists());
}

#[cfg(feature = "sqlite")]
#[test]
fn ensure_sqlite_parent_dir_handles_single_colon_prefix() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let nested = dir.path().join("colon_only/test.db");
    let url = format!("sqlite:{}", nested.display());
    ensure_sqlite_parent_dir(&url).expect("should create parent");
    assert!(dir.path().join("colon_only").exists());
}

#[cfg(feature = "sqlite")]
#[test]
fn ensure_sqlite_parent_dir_empty_path_skips() {
    ensure_sqlite_parent_dir("sqlite://").expect("empty path should skip mkdir");
}

#[cfg(feature = "sqlite")]
#[test]
fn ensure_sqlite_parent_dir_relative_no_dir_skips() {
    ensure_sqlite_parent_dir("sqlite://relative.db")
        .expect("relative path without dir prefix should skip mkdir");
}

#[tokio::test]
async fn create_pool_connection_failed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing = dir.path().join("definitely_missing.db");
    let url = format!("sqlite://{}?mode=ro", missing.display());
    let config = test_db_config(&url);
    let err = create_pool(&config).await.unwrap_err();
    assert!(
        matches!(err, DatabaseError::ConnectionFailed(_)),
        "got: {err}"
    );
}

#[tokio::test]
async fn run_migrations_with_bad_sql_returns_error() {
    let (pool, _dir) = test_pool_in_tempdir().await;
    let bad_dir = tempfile::tempdir().expect("bad tempdir");
    let bad_sql = bad_dir.path().join("20260507000000_bad.sql");
    std::fs::write(&bad_sql, "THIS IS NOT VALID SQL;").expect("write bad sql");
    let result = run_migrations(&pool, bad_dir.path()).await;
    assert!(matches!(result, Err(DatabaseError::MigrationFailed(_))));
    pool.close().await;
}

#[tokio::test]
async fn migration_counts_missing_dir_returns_error() {
    let (pool, _dir) = test_pool_in_tempdir().await;
    let result = migration_counts(&pool, Path::new("/nonexistent/path")).await;
    assert!(matches!(
        result,
        Err(DatabaseError::MigrationsDirectoryNotFound(_))
    ));
    pool.close().await;
}

#[tokio::test]
async fn migration_counts_query_failed_when_no_migration_table() {
    let (pool, _dir) = test_pool_in_tempdir().await;
    let migrations_dir = crate::test_helpers::workspace_migrations_dir();
    let result = migration_counts(&pool, &migrations_dir).await;
    assert!(matches!(result, Err(DatabaseError::QueryFailed(_))));
    pool.close().await;
}

#[tokio::test]
async fn db_status_snapshot_returns_health_even_when_migrations_fail() {
    let (pool, _dir) = test_pool_in_tempdir().await;
    let (health, counts_result) = db_status_snapshot(
        &pool,
        Path::new("/nonexistent/path"),
        Duration::from_secs(1),
    )
    .await;
    assert_eq!(health.status, "connected");
    assert!(matches!(
        counts_result,
        Err(DatabaseError::MigrationsDirectoryNotFound(_))
    ));
    pool.close().await;
}

#[tokio::test]
async fn db_status_snapshot_returns_health_and_counts_when_ok() {
    let (pool, _dir) = test_pool_in_tempdir().await;
    let migrations_dir = crate::test_helpers::workspace_migrations_dir();
    run_migrations(&pool, &migrations_dir)
        .await
        .expect("migrate");
    let (health, counts_result) =
        db_status_snapshot(&pool, &migrations_dir, Duration::from_secs(1)).await;
    assert_eq!(health.status, "connected");
    let counts = counts_result.expect("counts ok");
    assert_eq!(counts.applied, 1);
    assert_eq!(counts.total, 1);
    pool.close().await;
}
