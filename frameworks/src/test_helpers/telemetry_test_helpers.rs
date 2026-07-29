use super::TelemetryConfig;

pub fn telemetry_config(log_level: &str, log_format: &str) -> TelemetryConfig {
    TelemetryConfig {
        service_name: "skel_rs".to_string(),
        log_level: log_level.to_string(),
        log_format: log_format.to_string(),
    }
}
