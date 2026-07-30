use crate::AppConfig;

pub fn log_startup_banner(cfg: &AppConfig, version: &str, socket_path: &str) {
    let service = &cfg.telemetry.service_name;
    tracing::info!(
        target: "pm3::startup",
        feature = "lifecycle",
        operation = "startup",
        result = "ok",
        socket_path = %socket_path,
        home = %cfg.pm3.home,
        sandbox_mode = %cfg.pm3.sandbox.mode,
        sandbox_network = cfg.pm3.sandbox.network,
        log_level = %cfg.telemetry.log_level,
        log_format = %cfg.telemetry.log_format,
        "{service} v{version}",
    );
}
