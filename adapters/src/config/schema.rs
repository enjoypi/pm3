use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("cannot parse config: {0}")]
    ParseError(String),

    #[error("cannot accept empty server.host")]
    InvalidHost,

    #[error("cannot accept server.port 0")]
    InvalidPort,

    #[error("cannot accept server.drain_timeout_secs {0}: must be >= 1")]
    InvalidDrainTimeout(u64),

    #[error("cannot accept empty health_check.host")]
    InvalidHealthCheckHost,

    #[error("cannot accept health_check.connect_timeout_secs {0}: must be >= 1")]
    InvalidHealthCheckConnectTimeout(u64),

    #[error("cannot accept empty telemetry.service_name")]
    InvalidServiceName,

    #[error(
        "cannot accept telemetry.log_level {0}: must be one of trace, debug, info, warn, error"
    )]
    InvalidLogLevel(String),

    #[error("cannot accept telemetry.log_format {0}: must be one of json, pretty")]
    InvalidLogFormat(String),

    #[error("cannot accept empty database.url")]
    InvalidDatabaseUrl,

    #[error("cannot accept empty database.migrations_path")]
    InvalidMigrationsPath,

    #[error("cannot accept database.pool.max_connections {0}: must be >= 1")]
    InvalidMaxConnections(u32),

    #[error("cannot accept database.pool.min_connections {min}: must be <= max_connections {max}")]
    InvalidMinConnections { min: u32, max: u32 },

    #[error("cannot accept database.pool.acquire_timeout_secs {0}: must be >= 1")]
    InvalidAcquireTimeout(u64),

    #[error("cannot accept database.pool.health_check_timeout_secs {0}: must be >= 1")]
    InvalidHealthCheckTimeout(u64),

    #[error("cannot accept database.pool.idle_timeout_secs {0}: must be >= 1")]
    InvalidIdleTimeout(u64),

    #[error(
        "cannot accept database.pool.max_lifetime_secs {lifetime}: must be >= idle_timeout_secs {idle}"
    )]
    InvalidMaxLifetime { lifetime: u64, idle: u64 },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AppConfig {
    #[serde(default)]
    pub server: Option<ServerConfig>,
    pub telemetry: TelemetryConfig,
    #[serde(default)]
    pub database: Option<DatabaseConfig>,
    #[serde(default)]
    pub health_check: Option<HealthCheckConfig>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub migrations_path: String,
    pub pool: PoolConfig,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PoolConfig {
    pub max_connections: u32,
    pub min_connections: u32,
    pub acquire_timeout_secs: u64,
    pub idle_timeout_secs: u64,
    pub max_lifetime_secs: u64,
    pub health_check_timeout_secs: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub drain_timeout_secs: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct HealthCheckConfig {
    pub host: String,
    pub connect_timeout_secs: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TelemetryConfig {
    pub service_name: String,
    pub log_level: String,
    pub log_format: String,
}

pub const LOG_FORMAT_JSON: &str = "json";
pub const LOG_FORMAT_PRETTY: &str = "pretty";

const VALID_LOG_LEVELS: &[&str] = &["trace", "debug", "info", "warn", "error"];
const VALID_LOG_FORMATS: &[&str] = &[LOG_FORMAT_JSON, LOG_FORMAT_PRETTY];

pub fn validate_config(cfg: &AppConfig) -> Result<(), ConfigError> {
    if let Some(ref server) = cfg.server {
        validate_server_config(server)?;
    }

    validate_telemetry_config(&cfg.telemetry)?;

    if let Some(ref db) = cfg.database {
        validate_database_config(db)?;
    }

    if let Some(ref hc) = cfg.health_check {
        validate_health_check_config(hc)?;
    }

    Ok(())
}

pub fn validate_telemetry_config(t: &TelemetryConfig) -> Result<(), ConfigError> {
    if t.service_name.is_empty() {
        return Err(ConfigError::InvalidServiceName);
    }

    if !VALID_LOG_LEVELS.contains(&t.log_level.as_str()) {
        return Err(ConfigError::InvalidLogLevel(t.log_level.clone()));
    }

    if !VALID_LOG_FORMATS.contains(&t.log_format.as_str()) {
        return Err(ConfigError::InvalidLogFormat(t.log_format.clone()));
    }

    Ok(())
}

pub const fn validate_health_check_config(hc: &HealthCheckConfig) -> Result<(), ConfigError> {
    if hc.host.is_empty() {
        return Err(ConfigError::InvalidHealthCheckHost);
    }
    if hc.connect_timeout_secs < 1 {
        return Err(ConfigError::InvalidHealthCheckConnectTimeout(
            hc.connect_timeout_secs,
        ));
    }
    Ok(())
}

pub const fn validate_server_config(server: &ServerConfig) -> Result<(), ConfigError> {
    if server.host.is_empty() {
        return Err(ConfigError::InvalidHost);
    }

    if server.port == 0 {
        return Err(ConfigError::InvalidPort);
    }

    if server.drain_timeout_secs < 1 {
        return Err(ConfigError::InvalidDrainTimeout(server.drain_timeout_secs));
    }

    Ok(())
}

pub const fn validate_database_config(db: &DatabaseConfig) -> Result<(), ConfigError> {
    if db.url.is_empty() {
        return Err(ConfigError::InvalidDatabaseUrl);
    }

    if db.migrations_path.is_empty() {
        return Err(ConfigError::InvalidMigrationsPath);
    }

    let pool = &db.pool;

    if pool.max_connections < 1 {
        return Err(ConfigError::InvalidMaxConnections(pool.max_connections));
    }

    if pool.min_connections > pool.max_connections {
        return Err(ConfigError::InvalidMinConnections {
            min: pool.min_connections,
            max: pool.max_connections,
        });
    }

    if pool.acquire_timeout_secs < 1 {
        return Err(ConfigError::InvalidAcquireTimeout(
            pool.acquire_timeout_secs,
        ));
    }

    if pool.health_check_timeout_secs < 1 {
        return Err(ConfigError::InvalidHealthCheckTimeout(
            pool.health_check_timeout_secs,
        ));
    }

    if pool.idle_timeout_secs < 1 {
        return Err(ConfigError::InvalidIdleTimeout(pool.idle_timeout_secs));
    }

    if pool.max_lifetime_secs < pool.idle_timeout_secs {
        return Err(ConfigError::InvalidMaxLifetime {
            lifetime: pool.max_lifetime_secs,
            idle: pool.idle_timeout_secs,
        });
    }

    Ok(())
}

#[cfg(test)]
#[path = "../test_helpers/config_schema_test_helpers.rs"]
mod test_helpers;
#[cfg(test)]
#[path = "../tests/config_schema_tests.rs"]
mod tests;
