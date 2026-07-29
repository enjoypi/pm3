use super::*;

pub fn valid_pool_config() -> PoolConfig {
    PoolConfig {
        max_connections: 10,
        min_connections: 1,
        acquire_timeout_secs: 5,
        idle_timeout_secs: 300,
        max_lifetime_secs: 1800,
        health_check_timeout_secs: 3,
    }
}

pub fn valid_db_config() -> DatabaseConfig {
    DatabaseConfig {
        url: "sqlite://test.db".to_string(),
        migrations_path: "./migrations".to_string(),
        pool: valid_pool_config(),
    }
}

pub fn valid_config() -> AppConfig {
    AppConfig {
        server: Some(ServerConfig {
            host: "0.0.0.0".to_string(),
            port: 9229,
            drain_timeout_secs: 20,
        }),
        telemetry: TelemetryConfig {
            service_name: "skel_rs".to_string(),
            log_level: "info".to_string(),
            log_format: "json".to_string(),
        },
        database: None,
        health_check: Some(HealthCheckConfig {
            host: "127.0.0.1".to_string(),
            connect_timeout_secs: 2,
        }),
    }
}
