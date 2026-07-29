pub mod config;
#[cfg(has_database)]
pub mod database;
#[cfg(has_http)]
pub mod examples;
#[cfg(has_http)]
pub mod handlers;
#[cfg(has_http)]
pub mod middleware;
pub mod startup;
pub mod state;
#[cfg(feature = "sqlite")]
pub mod store;
pub mod time;

use thiserror::Error;
pub use usecases::{
    CreateExampleError, EntityError, Example, ExampleStore, StoreError, create_example,
    find_example,
};

#[cfg(has_database)]
pub use self::state::{DbConnection, DbReadinessPolicy};
#[cfg(feature = "sqlite")]
pub use self::store::SqlExampleStore;
pub use self::{
    config::{
        AppConfig, ConfigError, DatabaseConfig, HealthCheckConfig, LOG_FORMAT_JSON,
        LOG_FORMAT_PRETTY, PoolConfig, ServerConfig, TelemetryConfig, check_config,
        load_and_parse_config, parse_config, show_config, validate_config,
        validate_database_config, validate_health_check_config, validate_server_config,
        validate_telemetry_config,
    },
    startup::log_startup_banner,
    state::AppState,
    time::elapsed_ms,
};

#[derive(Debug, Error)]
pub enum AdapterError {
    #[error(transparent)]
    Config(#[from] config::ConfigLoadError),

    #[error(transparent)]
    Parse(#[from] ConfigError),

    #[cfg(has_http)]
    #[error(
        "cannot accept server section without health_check section (read by `health-check` CLI sub-command)"
    )]
    MissingHealthCheckSection,
}

pub type Result<T> = std::result::Result<T, AdapterError>;
