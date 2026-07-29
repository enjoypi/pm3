use std::time::Instant;

use usecases::{Example, ExampleStore, StoreError};

use crate::{database::DbPool, elapsed_ms};

#[derive(Clone)]
pub struct SqlExampleStore {
    pool: DbPool,
}

impl SqlExampleStore {
    #[must_use]
    pub const fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

impl ExampleStore for SqlExampleStore {
    async fn create(&self, name: &str) -> Result<Example, StoreError> {
        let started = Instant::now();
        let (id, name) = sqlx::query_as::<_, (i64, String)>(
            "INSERT INTO examples (name) VALUES (?) RETURNING id, name",
        )
        .bind(name)
        .fetch_one(&self.pool)
        .await
        .inspect_err(|e| log_query_failure("store.create", elapsed_ms(started), e))
        .map_err(|e| StoreError::Database(e.to_string()))?;

        let duration_ms = elapsed_ms(started);
        tracing::debug!(
            feature = "example",
            operation = "store.create",
            result = "ok",
            duration_ms,
            id = id,
            "example created in store",
        );
        Ok(Example { id, name })
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<Example>, StoreError> {
        let started = Instant::now();
        let row = sqlx::query_as::<_, (i64, String)>("SELECT id, name FROM examples WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .inspect_err(|e| log_query_failure("store.find_by_id", elapsed_ms(started), e))
            .map_err(|e| StoreError::Database(e.to_string()))?;

        let duration_ms = elapsed_ms(started);
        let found = row.is_some();
        tracing::debug!(
            feature = "example",
            operation = "store.find_by_id",
            result = "ok",
            duration_ms,
            id = id,
            found,
            "store query complete",
        );
        Ok(row.map(|(row_id, name)| Example { id: row_id, name }))
    }
}

fn log_query_failure(operation: &'static str, duration_ms: u64, error: &sqlx::Error) {
    tracing::warn!(
        feature = "example",
        operation,
        result = "error",
        duration_ms,
        error = %error,
        "store query failed",
    );
}

#[cfg(test)]
#[path = "test_helpers/store_test_helpers.rs"]
mod test_helpers;
#[cfg(test)]
#[path = "tests/store_tests.rs"]
mod tests;
