use thiserror::Error;

#[derive(Debug, Error)]
pub enum EntityError {
    #[error("cannot accept empty example name")]
    EmptyName,
}

pub type Result<T> = std::result::Result<T, EntityError>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Example {
    pub id: i64,
    pub name: String,
}

impl Example {
    pub fn validate_name(name: &str) -> Result<()> {
        if name.trim().is_empty() {
            return Err(EntityError::EmptyName);
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests/lib_tests.rs"]
mod tests;
