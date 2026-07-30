use super::TelemetryConfig;

pub fn telemetry_config(log_level: &str, log_format: &str) -> TelemetryConfig {
    TelemetryConfig {
        service_name: "pm3".to_string(),
        log_level: log_level.to_string(),
        log_format: log_format.to_string(),
    }
}
