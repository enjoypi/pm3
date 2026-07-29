use super::{test_helpers::*, *};
use crate::CreateExampleError;

#[tokio::test]
async fn create_example_propagates_store_error() {
    let store = MockStore::new();
    let err = create_example(&store, FAILING_NAME).await.unwrap_err();
    assert!(matches!(err, CreateExampleError::Store(_)), "got: {err}");
}

#[tokio::test]
async fn find_example_propagates_store_error() {
    let store = MockStore::new();
    let err = find_example(&store, FAILING_ID).await.unwrap_err();
    assert_eq!(err.to_string(), "cannot access database: connection reset");
}

#[tokio::test]
async fn create_example_valid() {
    let store = MockStore::new();
    let result = create_example(&store, "test")
        .await
        .expect("should succeed");
    assert_eq!(result.name, "test");
    assert_eq!(result.id, 1);
}

#[tokio::test]
async fn create_example_empty_name_rejected() {
    let store = MockStore::new();
    let err = create_example(&store, "").await.unwrap_err();
    assert!(err.to_string().contains("empty"), "got: {err}");
}

#[tokio::test]
async fn create_example_whitespace_name_rejected() {
    let store = MockStore::new();
    let err = create_example(&store, "   ").await.unwrap_err();
    assert!(err.to_string().contains("empty"), "got: {err}");
}

#[tokio::test]
async fn find_example_found() {
    let store = MockStore::new();
    let result = find_example(&store, 1).await.expect("should succeed");
    assert!(result.is_some());
    assert_eq!(result.expect("should be Some").name, "found");
}

#[tokio::test]
async fn find_example_not_found() {
    let store = MockStore::new();
    let result = find_example(&store, 999).await.expect("should succeed");
    assert!(result.is_none());
}
