#[cfg(has_database)]
use std::time::Duration;

#[cfg(has_database)]
use crate::database::DbPool;

#[cfg(has_database)]
#[derive(Clone)]
pub struct DbConnection {
    pub pool: DbPool,
    pub expected_migrations: i64,
    pub health_check_timeout: Duration,
}

#[cfg(has_database)]
#[derive(Copy, Clone, Debug)]
pub struct DbReadinessPolicy {
    pub expected_migrations: i64,
    pub health_check_timeout_secs: u64,
}

#[derive(Clone, Default)]
pub struct AppState {
    #[cfg(has_database)]
    pub db: Option<DbConnection>,
}

impl AppState {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            #[cfg(has_database)]
            db: None,
        }
    }

    #[cfg(has_database)]
    #[must_use]
    pub fn with_db_pool(mut self, pool: DbPool, policy: DbReadinessPolicy) -> Self {
        let DbReadinessPolicy {
            expected_migrations,
            health_check_timeout_secs,
        } = policy;
        self.db = Some(DbConnection {
            pool,
            expected_migrations,
            health_check_timeout: Duration::from_secs(health_check_timeout_secs),
        });
        self
    }
}
