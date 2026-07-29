use crate::{Example, ExampleStore, Result, StoreError};

#[tracing::instrument(
    skip(store),
    fields(feature = "example", operation = "create"),
    err(level = "warn")
)]
pub async fn create_example(store: &impl ExampleStore, name: &str) -> Result<Example> {
    Example::validate_name(name)?;
    Ok(store.create(name).await?)
}

#[tracing::instrument(
    skip(store),
    fields(feature = "example", operation = "find"),
    err(level = "warn")
)]
pub async fn find_example(
    store: &impl ExampleStore,
    id: i64,
) -> std::result::Result<Option<Example>, StoreError> {
    store.find_by_id(id).await
}

#[cfg(test)]
#[path = "test_helpers/example_test_helpers.rs"]
mod test_helpers;
#[cfg(test)]
#[path = "tests/example_tests.rs"]
mod tests;
