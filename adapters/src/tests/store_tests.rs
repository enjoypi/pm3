use super::{test_helpers::*, *};

#[tokio::test]
async fn create_and_find_example() {
    let (repo, _dir) = setup_test_store().await;
    let created = repo.create("test-item").await.expect("create");
    assert_eq!(created.name, "test-item");
    assert!(created.id > 0);

    let found = repo
        .find_by_id(created.id)
        .await
        .expect("find")
        .expect("should exist");
    assert_eq!(found, created);
}

#[tokio::test]
async fn find_by_id_not_found() {
    let (repo, _dir) = setup_test_store().await;
    let found = repo.find_by_id(999).await.expect("find");
    assert!(found.is_none());
}

#[tokio::test]
async fn create_returns_database_error_when_table_missing() {
    let (repo, _dir) = unmigrated_store().await;
    let err = repo.create("x").await.unwrap_err();
    assert!(matches!(err, StoreError::Database(_)));
}

#[tokio::test]
async fn find_by_id_returns_database_error_when_table_missing() {
    let (repo, _dir) = unmigrated_store().await;
    let err = repo.find_by_id(1).await.unwrap_err();
    assert!(matches!(err, StoreError::Database(_)));
}
