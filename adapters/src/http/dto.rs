use serde::{Deserialize, Serialize};

use super::view_dto::ProcessViewDto;

pub const HEALTH_OK: &str = "ok";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StartRequestDto {
    pub services: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SignalRequestDto {
    pub signal: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ReplyDto {
    pub report: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub already_running: Vec<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refused: Vec<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unsaved: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deleted: Vec<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub views: Vec<ProcessViewDto>,
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
