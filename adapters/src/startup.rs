use crate::AppConfig;

pub fn log_startup_banner(cfg: &AppConfig, version: &str) {
    let service = &cfg.telemetry.service_name;
    let server_addr = cfg.server.as_ref().map_or_else(
        || "disabled".to_string(),
        |s| format!("http://{}:{}", s.host, s.port),
    );
    tracing::info!(
        target: "skel_rs::startup",
        feature = "lifecycle",
        operation = "startup",
        result = "ok",
        server_addr = %server_addr,
        log_level = %cfg.telemetry.log_level,
        log_format = %cfg.telemetry.log_format,
        "{service} v{version}",
    );
}
