use super::SqlExampleStore;
use crate::database;

pub async fn setup_test_store() -> (SqlExampleStore, tempfile::TempDir) {
    let (pool, dir) = database::test_pool_in_tempdir().await;

    let migrations_dir = database::workspace_migrations_dir();
    sqlx::migrate::Migrator::new(migrations_dir.as_path())
        .await
        .expect("load migrator")
        .run(&pool)
        .await
        .expect("run migrations");

    (SqlExampleStore::new(pool), dir)
}

pub async fn unmigrated_store() -> (SqlExampleStore, tempfile::TempDir) {
    let (pool, dir) = database::test_pool_in_tempdir().await;
    (SqlExampleStore::new(pool), dir)
}
