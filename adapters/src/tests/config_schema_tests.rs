use super::{test_helpers::*, *};

#[test]
fn validate_no_server_section() {
    let mut cfg = valid_config();
    cfg.server = None;
    validate_config(&cfg).expect("server section should be optional");
}

#[test]
fn validate_valid_config() {
    validate_config(&valid_config()).expect("should validate");
}

#[test]
fn validate_empty_host() {
    let mut cfg = valid_config();
    cfg.server.as_mut().expect("server present").host = String::new();
    let err = validate_config(&cfg).unwrap_err();
    assert!(matches!(err, ConfigError::InvalidHost), "got: {err}");
}

#[test]
fn validate_port_zero() {
    let mut cfg = valid_config();
    cfg.server.as_mut().expect("server present").port = 0;
    let err = validate_config(&cfg).unwrap_err();
    assert!(matches!(err, ConfigError::InvalidPort), "got: {err}");
}

#[test]
fn validate_drain_timeout_zero() {
    let mut cfg = valid_config();
    cfg.server
        .as_mut()
        .expect("server present")
        .drain_timeout_secs = 0;
    let err = validate_config(&cfg).unwrap_err();
    assert!(
        matches!(err, ConfigError::InvalidDrainTimeout(0)),
        "got: {err}"
    );
}

#[test]
fn validate_empty_service_name() {
    let mut cfg = valid_config();
    cfg.telemetry.service_name = String::new();
    let err = validate_config(&cfg).unwrap_err();
    assert!(matches!(err, ConfigError::InvalidServiceName), "got: {err}");
}

#[test]
fn validate_invalid_log_level() {
    let mut cfg = valid_config();
    cfg.telemetry.log_level = "verbose".to_string();
    let err = validate_config(&cfg).unwrap_err();
    assert!(matches!(err, ConfigError::InvalidLogLevel(_)), "got: {err}");
}

#[test]
fn validate_invalid_log_format() {
    let mut cfg = valid_config();
    cfg.telemetry.log_format = "xml".to_string();
    let err = validate_config(&cfg).unwrap_err();
    assert!(
        matches!(err, ConfigError::InvalidLogFormat(_)),
        "got: {err}"
    );
}

#[test]
fn validate_telemetry_config_direct_valid() {
    let t = TelemetryConfig {
        service_name: "ok".to_string(),
        log_level: "info".to_string(),
        log_format: "json".to_string(),
    };
    validate_telemetry_config(&t).expect("ok");
}

#[test]
fn validate_telemetry_config_direct_empty_service_name() {
    let t = TelemetryConfig {
        service_name: String::new(),
        log_level: "info".to_string(),
        log_format: "json".to_string(),
    };
    assert!(matches!(
        validate_telemetry_config(&t),
        Err(ConfigError::InvalidServiceName)
    ));
}

#[test]
fn validate_database_empty_url() {
    let db = crate::config::DatabaseConfig {
        url: String::new(),
        ..valid_db_config()
    };
    let err = validate_database_config(&db).unwrap_err();
    assert!(matches!(err, ConfigError::InvalidDatabaseUrl), "got: {err}");
}

#[test]
fn validate_database_empty_migrations_path() {
    let db = crate::config::DatabaseConfig {
        migrations_path: String::new(),
        ..valid_db_config()
    };
    let err = validate_database_config(&db).unwrap_err();
    assert!(
        matches!(err, ConfigError::InvalidMigrationsPath),
        "got: {err}"
    );
}

#[test]
fn validate_database_max_less_than_one() {
    let db = crate::config::DatabaseConfig {
        pool: PoolConfig {
            max_connections: 0,
            ..valid_pool_config()
        },
        ..valid_db_config()
    };
    let err = validate_database_config(&db).unwrap_err();
    assert!(
        matches!(err, ConfigError::InvalidMaxConnections(0)),
        "got: {err}"
    );
}

#[test]
fn validate_database_min_greater_than_max() {
    let db = crate::config::DatabaseConfig {
        pool: PoolConfig {
            max_connections: 2,
            min_connections: 5,
            ..valid_pool_config()
        },
        ..valid_db_config()
    };
    let err = validate_database_config(&db).unwrap_err();
    assert!(
        matches!(err, ConfigError::InvalidMinConnections { .. }),
        "got: {err}"
    );
}

#[test]
fn validate_database_acquire_timeout_zero() {
    let db = crate::config::DatabaseConfig {
        pool: PoolConfig {
            acquire_timeout_secs: 0,
            ..valid_pool_config()
        },
        ..valid_db_config()
    };
    let err = validate_database_config(&db).unwrap_err();
    assert!(
        matches!(err, ConfigError::InvalidAcquireTimeout(0)),
        "got: {err}"
    );
}

#[test]
fn validate_database_health_check_timeout_zero() {
    let db = crate::config::DatabaseConfig {
        pool: PoolConfig {
            health_check_timeout_secs: 0,
            ..valid_pool_config()
        },
        ..valid_db_config()
    };
    let err = validate_database_config(&db).unwrap_err();
    assert!(
        matches!(err, ConfigError::InvalidHealthCheckTimeout(0)),
        "got: {err}"
    );
}

#[test]
fn validate_database_idle_timeout_zero() {
    let db = crate::config::DatabaseConfig {
        pool: PoolConfig {
            idle_timeout_secs: 0,
            max_lifetime_secs: 0,
            ..valid_pool_config()
        },
        ..valid_db_config()
    };
    let err = validate_database_config(&db).unwrap_err();
    assert!(
        matches!(err, ConfigError::InvalidIdleTimeout(0)),
        "got: {err}"
    );
}

#[test]
fn validate_database_lifetime_less_than_idle() {
    let db = crate::config::DatabaseConfig {
        pool: PoolConfig {
            idle_timeout_secs: 600,
            max_lifetime_secs: 300,
            ..valid_pool_config()
        },
        ..valid_db_config()
    };
    let err = validate_database_config(&db).unwrap_err();
    assert!(
        matches!(err, ConfigError::InvalidMaxLifetime { .. }),
        "got: {err}"
    );
}

#[test]
fn validate_database_valid() {
    validate_database_config(&valid_db_config()).expect("should validate");
}

#[test]
fn validate_config_with_database_section_runs_db_validation() {
    let mut cfg = valid_config();
    cfg.database = Some(valid_db_config());
    validate_config(&cfg).expect("should validate");
}

#[test]
fn validate_config_with_invalid_database_propagates_error() {
    let mut cfg = valid_config();
    cfg.database = Some(crate::config::DatabaseConfig {
        url: String::new(),
        ..valid_db_config()
    });
    assert!(matches!(
        validate_config(&cfg),
        Err(ConfigError::InvalidDatabaseUrl)
    ));
}

#[test]
fn validate_no_health_check_section() {
    let mut cfg = valid_config();
    cfg.health_check = None;
    validate_config(&cfg).expect("health_check section should be optional");
}

#[test]
fn validate_health_check_empty_host() {
    let mut cfg = valid_config();
    cfg.health_check = Some(HealthCheckConfig {
        host: String::new(),
        connect_timeout_secs: 2,
    });
    let err = validate_config(&cfg).unwrap_err();
    assert!(matches!(err, ConfigError::InvalidHealthCheckHost));
}

#[test]
fn validate_health_check_config_direct_empty_host() {
    let hc = HealthCheckConfig {
        host: String::new(),
        connect_timeout_secs: 2,
    };
    let err = validate_health_check_config(&hc).unwrap_err();
    assert!(matches!(err, ConfigError::InvalidHealthCheckHost));
}

#[test]
fn validate_health_check_config_direct_valid() {
    let hc = HealthCheckConfig {
        host: "127.0.0.1".to_string(),
        connect_timeout_secs: 2,
    };
    validate_health_check_config(&hc).expect("should validate");
}

#[test]
fn validate_health_check_connect_timeout_zero() {
    let hc = HealthCheckConfig {
        host: "127.0.0.1".to_string(),
        connect_timeout_secs: 0,
    };
    let err = validate_health_check_config(&hc).unwrap_err();
    assert!(
        matches!(err, ConfigError::InvalidHealthCheckConnectTimeout(0)),
        "got: {err}"
    );
}
