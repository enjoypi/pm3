use super::{test_helpers::*, *};

#[test]
fn init_telemetry_pretty_ok() {
    let _ = init_telemetry(&telemetry_config("info", "pretty"), LogSink::Stdout);
}

#[test]
fn init_telemetry_json_ok() {
    init_telemetry(&telemetry_config("debug", "json"), LogSink::Stdout)
        .expect("json formatter installs ok");
}

#[test]
fn init_telemetry_twice_keeps_existing_subscriber() {
    init_telemetry(&telemetry_config("info", "json"), LogSink::Stderr)
        .expect("first init installs subscriber");
    init_telemetry(&telemetry_config("debug", "pretty"), LogSink::Stderr)
        .expect("second init keeps the existing subscriber and stays ok");
}

#[test]
fn init_telemetry_invalid_level_returns_error() {
    let err =
        init_telemetry(&telemetry_config("mytarget=BOGUS", "json"), LogSink::Stderr).unwrap_err();
    assert!(
        matches!(err, TelemetryError::InvalidFilter(_)),
        "got: {err}"
    );
    assert!(err.to_string().contains("cannot parse log_level filter"));
}
