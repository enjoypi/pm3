use adapters::{LOG_FORMAT_PRETTY, TelemetryConfig};
use thiserror::Error;
use tracing_subscriber::{
    EnvFilter, Layer as _, Registry, fmt::writer::BoxMakeWriter, layer::SubscriberExt,
    util::SubscriberInitExt,
};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum LogSink {
    Stdout,
    Stderr,
}

#[derive(Debug, Error)]
pub enum TelemetryError {
    #[error("cannot parse log_level filter: {0}")]
    InvalidFilter(String),
}

pub fn init_cli_telemetry(cfg: &TelemetryConfig) {
    init_telemetry(cfg, LogSink::Stderr).ok();
}

pub fn init_telemetry(cfg: &TelemetryConfig, sink: LogSink) -> Result<(), TelemetryError> {
    let writer = match sink {
        LogSink::Stdout => BoxMakeWriter::new(std::io::stdout),
        LogSink::Stderr => BoxMakeWriter::new(std::io::stderr),
    };
    let fmt_layer = if cfg.log_format == LOG_FORMAT_PRETTY {
        tracing_subscriber::fmt::layer()
            .with_target(true)
            .with_writer(writer)
            .boxed()
    } else {
        tracing_subscriber::fmt::layer()
            .json()
            .with_target(true)
            .with_writer(writer)
            .boxed()
    };

    let filter = EnvFilter::try_new(&cfg.log_level)
        .map_err(|e| TelemetryError::InvalidFilter(e.to_string()))?;

    match Registry::default().with(filter).with(fmt_layer).try_init() {
        Ok(()) => tracing::debug!(
            feature = "lifecycle",
            action = "telemetry_init",
            result = "ok",
            log_level = %cfg.log_level,
            log_format = %cfg.log_format,
            "telemetry subscriber installed",
        ),
        Err(e) => tracing::warn!(
            feature = "lifecycle",
            action = "telemetry_init",
            result = "skipped",
            log_level = %cfg.log_level,
            log_format = %cfg.log_format,
            error = %e,
            "telemetry subscriber already installed, this config is ignored",
        ),
    }

    Ok(())
}

#[cfg(test)]
#[path = "test_helpers/telemetry_test_helpers.rs"]
mod test_helpers;
#[cfg(test)]
#[path = "tests/telemetry_tests.rs"]
mod tests;
