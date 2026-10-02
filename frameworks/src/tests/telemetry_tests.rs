use super::{test_helpers::*, *};

#[test]
fn init_telemetry_pretty_ok() {
    init_telemetry(&telemetry_config("info", "pretty"), LogSink::Stdout);
}

#[test]
fn init_telemetry_json_ok() {
    init_telemetry(&telemetry_config("debug", "json"), LogSink::Stdout);
}

#[test]
fn init_telemetry_twice_keeps_existing_subscriber() {
    init_telemetry(&telemetry_config("info", "json"), LogSink::Stderr);
    init_telemetry(&telemetry_config("debug", "pretty"), LogSink::Stderr);
}
