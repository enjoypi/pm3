use super::{test_helpers::*, *};

#[test]
fn init_telemetry_pretty_ok() {
    let _ = init_telemetry(&telemetry_config("info", "pretty"));
}

#[test]
fn init_telemetry_json_ok() {
    init_telemetry(&telemetry_config("debug", "json")).expect("json formatter installs ok");
}

#[test]
fn init_telemetry_twice_keeps_existing_subscriber() {
    init_telemetry(&telemetry_config("info", "json")).expect("first init installs subscriber");
    init_telemetry(&telemetry_config("debug", "pretty"))
        .expect("second init keeps the existing subscriber and stays ok");
}

#[test]
fn init_telemetry_invalid_level_returns_error() {
    let err = init_telemetry(&telemetry_config("mytarget=BOGUS", "json")).unwrap_err();
    assert!(
        matches!(err, TelemetryError::InvalidFilter(_)),
        "got: {err}"
    );
    assert!(err.to_string().contains("cannot parse log_level filter"));
}
