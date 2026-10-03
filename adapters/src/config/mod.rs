pub mod app;
mod loader;
mod schema;
mod validate;

pub const DEFAULT_CONFIG: &str = include_str!("../../../config.yaml");

pub use self::{
    app::{check_config, load_and_parse_config, load_config_file, show_config},
    loader::{ConfigLoadError, load_config, substitute_env_vars},
    schema::{
        AppConfig, ConfigError, LOG_FORMAT_PRETTY, Pm3Config, RESTART_CONDITION_ON_FAILURE,
        RestartConfig, STOP_SIGNAL_TERM, SandboxConfig, ServiceConfig, TelemetryConfig,
    },
    validate::validate_config,
};
