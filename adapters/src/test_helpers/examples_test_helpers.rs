use super::*;

#[path = "../../test_support/response_body.rs"]
mod response_body;
#[path = "../../../usecases/test_support/store_failures.rs"]
mod store_failures;

pub use self::response_body::body_json;
use self::store_failures::STORE_FAILURE_REASON;

pub const FOUND_ID: i64 = 1;
pub const FOUND_NAME: &str = "found";

#[derive(Clone)]
pub struct MockStore {
    pub unreachable_store: bool,
}

impl MockStore {
    pub const fn working() -> Self {
        Self {
            unreachable_store: false,
        }
    }

    pub const fn unreachable() -> Self {
        Self {
            unreachable_store: true,
        }
    }

    fn database_error() -> StoreError {
        StoreError::Database(STORE_FAILURE_REASON.to_string())
    }
}

impl ExampleStore for MockStore {
    async fn create(&self, name: &str) -> Result<Example, StoreError> {
        if self.unreachable_store {
            return Err(Self::database_error());
        }
        Ok(Example {
            id: FOUND_ID,
            name: name.to_string(),
        })
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<Example>, StoreError> {
        if self.unreachable_store {
            return Err(Self::database_error());
        }
        if id == FOUND_ID {
            return Ok(Some(Example {
                id: FOUND_ID,
                name: FOUND_NAME.to_string(),
            }));
        }
        Ok(None)
    }
}
