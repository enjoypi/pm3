use std::future::Future;

use crate::{Example, StoreError};

pub trait ExampleStore: Send + Sync {
    fn create(&self, name: &str) -> impl Future<Output = Result<Example, StoreError>> + Send;

    fn find_by_id(
        &self,
        id: i64,
    ) -> impl Future<Output = Result<Option<Example>, StoreError>> + Send;
}
