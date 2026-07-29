use super::{
    AppConfig, ConfigError, load_config, redact_url, substitute_env_vars, validate_config,
};
#[cfg(has_http)]
use crate::AdapterError;
use crate::Result;

pub fn parse_config(yaml: &str) -> std::result::Result<AppConfig, ConfigError> {
    serde_yaml2::from_str(yaml).map_err(|e| ConfigError::ParseError(e.to_string()))
}

pub fn load_and_parse_config(path: &str) -> Result<AppConfig> {
    let raw = load_config(path)?;
    let substituted = substitute_env_vars(&raw)?;
    let cfg = parse_config(&substituted)?;
    validate_config(&cfg)?;

    #[cfg(has_http)]
    if cfg.server.is_some() && cfg.health_check.is_none() {
        return Err(AdapterError::MissingHealthCheckSection);
    }

    Ok(cfg)
}

pub fn check_config(path: &str) -> Result<String> {
    load_and_parse_config(path)?;
    Ok(format!("Config OK: {path}"))
}

#[expect(
    clippy::unwrap_in_result,
    reason = "AppConfig 序列化 Err 不可达（纯数据 derive Serialize），expect 消除永假 `?` region"
)]
pub fn show_config(path: &str) -> Result<String> {
    let mut cfg = load_and_parse_config(path)?;
    if let Some(db) = cfg.database.as_mut() {
        db.url = redact_url(&db.url);
    }
    let yaml = serde_yaml2::to_string(&cfg)
        .expect("internal error: AppConfig serialization is infallible");
    Ok(yaml)
}

#[cfg(test)]
#[path = "../test_helpers/config_app_test_helpers.rs"]
mod test_helpers;
#[cfg(test)]
#[path = "../tests/config_app_tests.rs"]
mod tests;
