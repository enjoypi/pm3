use serde::{Deserialize, Serialize};

pub const HEALTH_OK: &str = "ok";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StartRequestDto {
    pub apps_file: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct HealthDto {
    pub status: String,
}

impl HealthDto {
    #[must_use]
    pub fn healthy() -> Self {
        Self {
            status: HEALTH_OK.to_string(),
        }
    }
}
