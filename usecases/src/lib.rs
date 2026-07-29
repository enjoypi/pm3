pub mod example;
pub mod store;

pub use entities::{EntityError, Example};
use thiserror::Error;

pub use self::{
    example::{create_example, find_example},
    store::ExampleStore,
};

#[derive(Debug, Error)]
pub enum CreateExampleError {
    #[error(transparent)]
    Validation(#[from] EntityError),

    #[error(transparent)]
    Store(#[from] StoreError),
}

pub type Result<T> = std::result::Result<T, CreateExampleError>;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("cannot access database: {0}")]
    Database(String),
}
