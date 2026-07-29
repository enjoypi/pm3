use adapters::PoolConfig;

use super::*;

pub fn test_db_config(url: &str) -> DatabaseConfig {
    DatabaseConfig {
        url: url.to_string(),
        migrations_path: "./migrations".to_string(),
        pool: PoolConfig {
            max_connections: 10,
            min_connections: 1,
            acquire_timeout_secs: 5,
            idle_timeout_secs: 300,
            max_lifetime_secs: 1800,
            health_check_timeout_secs: 3,
        },
    }
}

pub async fn test_pool_in_tempdir() -> (DbPool, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("create temp dir");
    let url = crate::test_helpers::sqlite_rwc_url(&dir.path().join("test.db"));
    let config = test_db_config(&url);
    let pool = create_pool(&config).await.expect("create pool");
    (pool, dir)
}
