use super::{AppConfig, ConfigError, load_config, substitute_env_vars, validate_config};
use crate::Result;

#[derive(Clone, Debug)]
pub struct LoadedConfig {
    pub source: String,
    pub config: AppConfig,
}

pub fn parse_config(yaml: &str) -> std::result::Result<AppConfig, ConfigError> {
    serde_yaml2::from_str(yaml).map_err(|e| ConfigError::ParseError(e.to_string()))
}

pub fn load_config_file(path: &str) -> Result<LoadedConfig> {
    let source = load_config(path)?;
    let substituted = substitute_env_vars(&source)?;
    let config = parse_config(&substituted)?;
    validate_config(&config)?;
    Ok(LoadedConfig { source, config })
}

pub fn load_and_parse_config(path: &str) -> Result<AppConfig> {
    Ok(load_config_file(path)?.config)
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
    let cfg = load_and_parse_config(path)?;
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
