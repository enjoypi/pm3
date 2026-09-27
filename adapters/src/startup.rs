use crate::AppConfig;

pub fn log_startup_banner(cfg: &AppConfig, version: &str, socket_path: &str) {
    let service = cfg.telemetry.service_name.as_str();
    let home = cfg.pm3.home.as_str();
    let sandbox_mode = cfg.pm3.sandbox.mode.as_str();
    let sandbox_network = cfg.pm3.sandbox.network;
    let log_level = cfg.telemetry.log_level.as_str();
    let log_format = cfg.telemetry.log_format.as_str();
    tracing::info!(
        target: "pm3::startup",
        feature = "lifecycle",
        action = "startup",
        result = "ok",
        service,
        version,
        socket_path,
        home,
        sandbox_mode,
        sandbox_network,
        log_level,
        log_format,
        "pm3 daemon is up",
    );
}

#[cfg(test)]
#[path = "tests/startup_tests.rs"]
mod tests;
