use tokio::sync::Mutex;

use super::*;

#[path = "../../test_support/store_failures.rs"]
mod store_failures;

use self::store_failures::STORE_FAILURE_REASON;

pub const FAILING_NAME: &str = "trigger-store-failure";
pub const FAILING_ID: i64 = -1;

pub struct MockStore {
    next_id: Mutex<i64>,
}

impl MockStore {
    pub fn new() -> Self {
        Self {
            next_id: Mutex::new(1),
        }
    }

    fn database_error() -> StoreError {
        StoreError::Database(STORE_FAILURE_REASON.to_string())
    }
}

impl ExampleStore for MockStore {
    async fn create(&self, name: &str) -> std::result::Result<Example, StoreError> {
        if name == FAILING_NAME {
            return Err(Self::database_error());
        }
        let id = {
            let mut guard = self.next_id.lock().await;
            let id = *guard;
            *guard += 1;
            id
        };
        Ok(Example {
            id,
            name: name.to_string(),
        })
    }

    async fn find_by_id(&self, id: i64) -> std::result::Result<Option<Example>, StoreError> {
        if id == FAILING_ID {
            return Err(Self::database_error());
        }
        if id == 1 {
            Ok(Some(Example {
                id: 1,
                name: "found".to_string(),
            }))
        } else {
            Ok(None)
        }
    }
}
